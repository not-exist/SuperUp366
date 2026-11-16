//! Error-notebook correction ("订正"): draw, status and the gzip submit.
//!
//! The official answer page is a webview; it draws a fresh paper from
//! `personalmistake/correction/draw`, then hands the result to the Electron main
//! process, which gzips the JSON body and posts it to
//! `study-api/client/task/score/gzip/submit` with `submitType = 11`.
//!
//! Read/write boundary:
//!
//! - `correction/draw` only creates a batch; it is safe to call repeatedly.
//! - `correction/status` is a read: the answer page polls it until marking ends.
//! - `task_score_gzip_submit` marks the drawn questions corrected. It is a real
//!   write and is never retried by the transport.
//!
//! This module builds the submission from the paper the server just returned,
//! so every question keeps the exact answer element the server authored.

use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::client::Client;
use crate::codec;
use crate::dto::flex;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};
use crate::signature::submit_task_ut;

/// Submission branch the correction answer page uses.
pub const CORRECTION_SUBMIT_TYPE: i64 = 11;

/// One blank or sub-answer of a correction question.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CorrectionAnswer {
    #[serde(deserialize_with = "text_or_empty")]
    pub id: String,
    /// Points this blank is worth.
    #[serde(default, deserialize_with = "number_or_zero")]
    pub score: f64,
    /// The correct text, as the server wrote it.
    #[serde(default)]
    pub content: String,
}

/// One option of a choice question.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CorrectionOption {
    pub id: String,
    pub text: String,
}

/// One question of a drawn correction paper.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CorrectionQuestion {
    pub question_id: String,
    /// `question_no` from the paper.
    pub number: String,
    /// Numeric question type, kept as text to preserve the wire value.
    pub question_type: String,
    pub type_name: String,
    pub score: f64,
    pub stem: String,
    pub answers: Vec<CorrectionAnswer>,
    pub options: Vec<CorrectionOption>,
}

/// A drawn correction paper plus the documents the submit needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CorrectionPaper {
    pub batch_id: String,
    pub path_prefix: String,
    pub paper_xml: String,
    pub answer_xml: String,
    pub questions: Vec<CorrectionQuestion>,
}

impl CorrectionPaper {
    /// Sum of every question's declared score.
    pub fn total_score(&self) -> f64 {
        self.questions.iter().map(|question| question.score).sum()
    }

    /// The correct text of one question, blanks joined the same way the
    /// wrong-note page shows them.
    ///
    /// Choice answers arrive as letters (`ABC`); expanding them against the
    /// paper's options is what makes the preview readable.
    pub fn answer_text(&self, question_id: &str) -> String {
        self.questions
            .iter()
            .find(|question| question.question_id == question_id)
            .map(|question| {
                let joined = question
                    .answers
                    .iter()
                    .map(|answer| answer.content.as_str())
                    .collect::<Vec<_>>()
                    .join("；");
                expand_options(&joined, &question.options)
            })
            .unwrap_or_default()
    }
}

/// Overview counts and the per-question-type groups.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionOverview {
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub remaining_count: i64,
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub corrected_count: i64,
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub today_corrected_count: i64,
    #[serde(default)]
    pub group_list: Vec<CorrectionGroup>,
}

/// One overview group: a question type (and knowledge point) with a count.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionGroup {
    #[serde(default, deserialize_with = "text_or_empty")]
    pub group_key: String,
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub remaining_count: i64,
    // The overview spells these `questionTypeName` / `knowledgePointName`.
    #[serde(
        default,
        rename = "questionTypeName",
        deserialize_with = "text_or_empty"
    )]
    pub question_type_name: String,
    #[serde(
        default,
        rename = "knowledgePointName",
        deserialize_with = "text_or_empty"
    )]
    pub knowledge_point_name: String,
}

/// Where correction marking stands for one batch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionStatus {
    /// `0` not submitted, `1` marking, `2` report ready.
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub status: i64,
    #[serde(default, deserialize_with = "number_or_zero")]
    pub percent: f64,
    #[serde(default, deserialize_with = "number_or_zero_i64")]
    pub submit_date: i64,
}

/// Context the submit needs that the draw response does not carry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CorrectionContext {
    pub uid: String,
    pub client_id: String,
    /// `-1` when the page was not opened from a job.
    pub course_id: i64,
    pub study_task_id: String,
}

/// The correction overview (remaining / corrected / groups).
pub async fn correction_overview(
    client: &Client,
    uid: i64,
    organ_id: i64,
    stage_id: i64,
) -> Result<CorrectionOverview> {
    let endpoint = endpoint("pm_overview")?;
    let data = client
        .call(
            endpoint,
            &[
                ("studentId", json!(uid)),
                ("organId", json!(organ_id)),
                ("stageId", json!(stage_id)),
            ],
        )
        .await?;
    let mut object = codec::into_object(data, endpoint.name)?;
    // The groups are the only list; decode the scalar counters tolerantly.
    let groups = match object.remove("groupList") {
        Some(Value::Array(items)) => items
            .into_iter()
            .map(|item| decode(item, endpoint.name))
            .collect::<Result<Vec<_>>>()?,
        _ => Vec::new(),
    };
    let overview = decode(Value::Object(object), endpoint.name)?;
    Ok(CorrectionOverview {
        group_list: groups,
        ..overview
    })
}

/// Draw a correction paper.
///
/// `question_ids` selects exact questions; when empty the server draws
/// `count` questions (optionally from one overview `group_key`).
pub async fn correction_draw(
    client: &Client,
    student_id: i64,
    question_ids: &[String],
    count: i64,
    group_key: Option<&str>,
) -> Result<Option<CorrectionPaper>> {
    let endpoint = endpoint("pm_correction_draw")?;
    let mut fields: Vec<(&str, Value)> = vec![
        ("studentId", json!(student_id)),
        ("questionCount", json!(count)),
    ];
    if !question_ids.is_empty() {
        fields.push(("questionIdList", json!(question_ids)));
    }
    if let Some(group_key) = group_key.filter(|key| !key.trim().is_empty()) {
        fields.push(("groupKey", json!(group_key)));
    }
    let data = client.call(endpoint, &fields).await?;
    let object = match data {
        Some(Value::Object(object)) if !object.is_empty() => object,
        _ => return Ok(None),
    };
    correction_from_object(object, endpoint.name).map(Some)
}

/// Poll-style status read for one correction batch.
pub async fn correction_status(
    client: &Client,
    batch_id: &str,
    uid: &str,
) -> Result<CorrectionStatus> {
    let endpoint = endpoint("pm_correction_status")?;
    let data = client
        .call(
            endpoint,
            &[("batchId", json!(batch_id)), ("uid", json!(uid))],
        )
        .await?;
    let object = match data {
        Some(Value::Object(object)) => object,
        other => codec::into_object(other, endpoint.name)?,
    };
    let status = decode(Value::Object(object), endpoint.name)?;
    Ok(status)
}

/// Decode the draw payload and merge the answer document into the questions.
fn correction_from_object(object: Map<String, Value>, endpoint: &str) -> Result<CorrectionPaper> {
    let batch_id = string_field(&object, "batchId")?;
    let path_prefix = string_field(&object, "pathPrefix").unwrap_or_default();
    let paper_xml = string_field(&object, "paperData")?;
    let answer_xml = string_field(&object, "answerData").unwrap_or_default();
    let mut questions = questions_from_paper(&paper_xml);
    let answers = answers_from_document(&answer_xml);
    for question in &mut questions {
        question.answers = answers
            .get(&question.question_id)
            .cloned()
            .unwrap_or_default();
    }
    if questions.is_empty() {
        return Err(ProtocolError::Decode(format!(
            "{endpoint}: paper contained no questions"
        )));
    }
    Ok(CorrectionPaper {
        batch_id,
        path_prefix,
        paper_xml,
        answer_xml,
        questions,
    })
}

/// The `qstJson` array the submit carries.
///
/// One `elementType = 1` row per question and one `elementType = 2` row per
/// blank, mirroring the shape the official answer page serialises.
pub fn qst_json(paper: &CorrectionPaper) -> Value {
    let mut rows = Vec::new();
    for question in &paper.questions {
        let question_type = number_text(&question.question_type);
        // Choice questions carry the answer on the parent row; blank-scored
        // types carry it on their per-blank rows instead.
        let parent_answer = if marks_per_blank(&question.question_type) {
            String::new()
        } else {
            raw_answer(question)
        };
        rows.push(json!({
            "elementId": question.question_id,
            "elementType": 1,
            "elementAttr": {
                "question_id": question.question_id,
                "question_type": question_type,
                "order": 1,
                "user_score": question.score,
                "user_answer": parent_answer,
                "answer": "",
                "score": question.score,
                "result": 1,
                "timestamp": 0,
                "record_url": "",
                "duration": 0,
                "wrongnote_flag": 1,
                "recForm": 1,
                "comment": "",
                "comment_ai": "",
                "pic_url": "",
                "ocrTids": "",
                "tScore": Value::Null,
            }
        }));
        // Only per-blank types serialise sub-question rows; a choice question's
        // `ABC` lives on the parent row.
        if marks_per_blank(&question.question_type) {
            for answer in &question.answers {
                rows.push(json!({
                    "elementId": format!("{}-{}", question.question_id, answer.id),
                    "elementType": 2,
                    "elementAttr": {
                        "question_id": question.question_id,
                        "question_type": question_type,
                        "order": answer.id.parse::<i64>().unwrap_or_default(),
                        "user_score": answer.score,
                        "user_answer": answer.content,
                        "timestamp": 0,
                        "answer": answer.content,
                        "score": answer.score,
                        "result": 1,
                    }
                }));
            }
        }
    }
    Value::Array(rows)
}

/// `xmlResult.markdatao`: every question marked fully correct.
///
/// Question types that mark per blank (`4 填空`, `6 连线`, `8 拖拽`, `10 语法纠错`,
/// `11 M选N`) carry no parent `<mark_data>`; their blanks do, including the
/// answer text. Everything else gets one parent mark worth the question score.
pub fn mark_xml(paper: &CorrectionPaper) -> String {
    let mut elements = String::from("<elements>");
    for question in &paper.questions {
        elements.push_str("<element id=\"");
        elements.push_str(&escape(&question.question_id));
        elements.push_str("\">");

        if marks_per_blank(&question.question_type) {
            for answer in &question.answers {
                elements.push_str(&mark_data(&answer.id, answer.score, Some(&answer.content)));
            }
        } else {
            elements.push_str(&mark_data("1", question.score, None));
        }
        elements.push_str("</element>");
    }
    elements.push_str("</elements>");
    elements
}

fn mark_data(id: &str, score: f64, user_answer: Option<&str>) -> String {
    let mut xml = format!(
        "<mark_data id=\"{}\"><score>{}</score><result>1</result><teacher_marked>0</teacher_marked>",
        escape(id),
        score
    );
    if let Some(answer) = user_answer {
        xml.push_str("<user_answer><![CDATA[");
        xml.push_str(&answer.replace("]]>", "]]]]><![CDATA[>"));
        xml.push_str("]]></user_answer>");
    }
    xml.push_str("</mark_data>");
    xml
}

fn marks_per_blank(question_type: &str) -> bool {
    matches!(question_type.trim(), "4" | "6" | "8" | "10" | "11")
}

fn raw_answer(question: &CorrectionQuestion) -> String {
    question
        .answers
        .iter()
        .map(|answer| answer.content.as_str())
        .collect::<Vec<_>>()
        .join("；")
}

/// Turn `ABC` into `A. 选项一；B. 选项二；C. 选项三` when the paper has options.
fn expand_options(answer: &str, options: &[CorrectionOption]) -> String {
    let letters: Vec<char> = answer.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    if options.is_empty() || letters.len() != answer.chars().count() || letters.is_empty() {
        return answer.to_owned();
    }
    let mut parts = Vec::with_capacity(letters.len());
    for letter in letters {
        match options
            .iter()
            .find(|option| option.id.eq_ignore_ascii_case(&letter.to_string()))
        {
            Some(option) => parts.push(format!("{letter}. {}", option.text)),
            None => return answer.to_owned(),
        }
    }
    parts.join("；")
}

/// Build the single `tasksJson` object the gzip submit carries.
pub fn build_tasks_json(
    paper: &CorrectionPaper,
    context: &CorrectionContext,
    seconds: i64,
    study_date: i64,
    wrongnote_flag: &str,
) -> Value {
    json!({
        "uid": context.uid,
        "killStudyTask": 0,
        "batchId": paper.batch_id,
        "bookId": "",
        "studyTaskId": context.study_task_id,
        "courseId": context.course_id,
        "taskId": "",
        "chapterId": "",
        "paperId": "",
        "bookType": Value::Null,
        "seconds": seconds,
        "percent": 100,
        "score": paper.total_score(),
        "testId": paper.batch_id,
        "extendParams": {
            "batchId": paper.batch_id,
            "paperDataXml": paper.paper_xml,
            "answerDataXml": paper.answer_xml,
            "xmlResult": {
                "answerdata": paper.answer_xml,
                "markdatao": mark_xml(paper),
            },
        },
        "taskNo": 1,
        "qstJson": qst_json(paper),
        "deviceType": "",
        "clientId": context.client_id,
        "studyDate": study_date,
        "wrongnoteFlag": wrongnote_flag,
    })
}

/// Send one correction submission. The caller must have previewed it.
///
/// `tasks` is the object from [`build_tasks_json`]; the endpoint expects it in
/// a one-element array, signed with the nested task-score digest.
pub async fn submit_correction(
    client: &Client,
    tasks: &Value,
    timestamp_ms: u128,
) -> Result<Value> {
    let endpoint = endpoint("task_score_gzip_submit")?;
    let tasks_json = serde_json::to_string(&json!([tasks]))
        .map_err(|error| ProtocolError::Decode(format!("tasksJson: {error}")))?;
    let ut = submit_task_ut(&tasks_json, timestamp_ms);
    let data = client
        .call(
            endpoint,
            &[
                ("tasksJson", json!(tasks_json)),
                ("ut", json!(ut)),
                ("submitType", json!(CORRECTION_SUBMIT_TYPE)),
            ],
        )
        .await?;
    Ok(data.unwrap_or(Value::Null))
}

/// "错题消灭": associate the cleared questions with their study task.
///
/// Only meaningful when the correction was opened from a job; it is a separate
/// write, so a failure here must not hide the successful submit.
pub async fn clear_wrong_notes(
    client: &Client,
    uid: &str,
    course_id: i64,
    study_task_id: &str,
) -> Result<Value> {
    let endpoint = endpoint("job_submit")?;
    let job_json = json!({
        "uid": uid,
        "courseId": course_id,
        "studyTaskId": study_task_id,
    })
    .to_string();
    let data = client
        .call(endpoint, &[("jobJson", json!(job_json))])
        .await?;
    Ok(data.unwrap_or(Value::Null))
}

/// Look up an endpoint by name.
fn endpoint(name: &str) -> Result<&'static crate::endpoint::Endpoint> {
    get(name).ok_or_else(|| ProtocolError::Decode(format!("missing endpoint: {name}")))
}

fn decode<T>(value: Value, endpoint: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value)
        .map_err(|error| ProtocolError::Decode(format!("{endpoint}: {error}")))
}

fn string_field(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ProtocolError::Decode(format!("missing field: {key}")))
}

/// A numeric question type stays a JSON number; anything else becomes its text.
fn number_text(text: &str) -> Value {
    match text.trim().parse::<i64>() {
        Ok(number) => json!(number),
        Err(_) => json!(0),
    }
}

#[derive(Debug, Default)]
struct PartialQuestion {
    question_id: String,
    number: String,
    question_type: String,
    type_name: String,
    score: f64,
    stem: String,
    options: Vec<CorrectionOption>,
    saw_question: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Collecting {
    Nothing,
    Stem,
    Number,
    Type,
    TypeName,
    Score,
    Option,
}

/// Lift the questions out of the paper document.
///
/// The draw paper is flat: every `<element>` with a `<question_no>` (or a
/// `<question_text>`) is one question. Malformed XML keeps what was read.
fn questions_from_paper(xml: &str) -> Vec<CorrectionQuestion> {
    let mut questions = Vec::new();
    if xml.trim().is_empty() {
        return questions;
    }

    let mut reader = Reader::from_str(xml);
    let mut current: Option<PartialQuestion> = None;
    let mut depth = 0usize;
    let mut collecting = Collecting::Nothing;
    let mut buffer = String::new();
    let mut option_id: Option<String> = None;

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match event {
            Event::Start(start) => {
                if local_name(&start) == "element" {
                    if current.is_none() {
                        current = Some(PartialQuestion {
                            question_id: attribute(&start, "id").unwrap_or_default(),
                            ..Default::default()
                        });
                    }
                    depth += 1;
                    continue;
                }
                if current.is_none() {
                    continue;
                }
                if local_name(&start) == "option" {
                    option_id = attribute(&start, "id");
                    collecting = Collecting::Option;
                    buffer.clear();
                    continue;
                }
                let next = match local_name(&start) {
                    "question_text" => Collecting::Stem,
                    "question_no" => Collecting::Number,
                    "question_type" => Collecting::Type,
                    "qlib_qst_type_name" => Collecting::TypeName,
                    "question_score" => Collecting::Score,
                    _ => Collecting::Nothing,
                };
                if next != Collecting::Nothing {
                    collecting = next;
                    buffer.clear();
                }
            }
            Event::Empty(start) => {
                if local_name(&start) == "element" && current.is_none() {
                    // A self-closing element carries no question.
                    continue;
                }
            }
            Event::End(end) => {
                if end.name().local_name().into_inner() == "element" {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if let Some(question) = current.take() {
                            if question.saw_question {
                                questions.push(CorrectionQuestion {
                                    question_id: question.question_id,
                                    number: question.number,
                                    question_type: question.question_type,
                                    type_name: question.type_name,
                                    score: question.score,
                                    stem: plain_text(&question.stem),
                                    answers: Vec::new(),
                                    options: question.options,
                                });
                            }
                        }
                        collecting = Collecting::Nothing;
                    }
                    continue;
                }
                if let Some(question) = current.as_mut() {
                    match collecting {
                        Collecting::Stem => {
                            question.stem = buffer.clone();
                            question.saw_question = true;
                        }
                        Collecting::Number => {
                            question.number = buffer.trim().to_owned();
                            question.saw_question = true;
                        }
                        Collecting::Type => question.question_type = buffer.trim().to_owned(),
                        Collecting::TypeName => question.type_name = buffer.trim().to_owned(),
                        Collecting::Score => {
                            question.score = buffer.trim().parse().unwrap_or_default();
                            question.saw_question = true;
                        }
                        Collecting::Option => {
                            if let Some(id) = option_id.take() {
                                question.options.push(CorrectionOption {
                                    id,
                                    text: plain_text(&buffer),
                                });
                            }
                        }
                        Collecting::Nothing => {}
                    }
                }
                collecting = Collecting::Nothing;
            }
            Event::Text(text) if collecting != Collecting::Nothing => buffer.push_str(&text),
            Event::CData(text) if collecting != Collecting::Nothing => buffer.push_str(&text),
            _ => {}
        }
    }
    questions
}

#[derive(Debug, Default)]
struct PartialAnswer {
    id: String,
    score: f64,
    content: String,
}

/// The correct answers, keyed by question id and then answer id.
fn answers_from_document(xml: &str) -> HashMap<String, Vec<CorrectionAnswer>> {
    let mut answers: HashMap<String, Vec<CorrectionAnswer>> = HashMap::new();
    if xml.trim().is_empty() {
        return answers;
    }

    let mut reader = Reader::from_str(xml);
    let mut element_id: Option<String> = None;
    let mut current: Option<PartialAnswer> = None;
    let mut collecting = false;
    let mut buffer = String::new();
    // Choice questions store their answer as `<answers>ABC</answers>` rather
    // than as `<answer>` children, so the container's own text is collected too.
    let mut group_collecting = false;
    let mut group_buffer = String::new();

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match event {
            Event::Start(start) => match local_name(&start) {
                "element" => element_id = attribute(&start, "id"),
                "answers" => {
                    group_collecting = true;
                    group_buffer.clear();
                }
                "answer" => {
                    group_collecting = false;
                    current = Some(PartialAnswer {
                        id: attribute(&start, "id").unwrap_or_default(),
                        score: attribute(&start, "score")
                            .and_then(|score| score.parse().ok())
                            .unwrap_or_default(),
                        content: String::new(),
                    });
                    collecting = true;
                    buffer.clear();
                }
                _ => {}
            },
            Event::End(end) => {
                match end.name().local_name().into_inner() {
                    "element" => element_id = None,
                    "answers" => {
                        group_collecting = false;
                        let content = group_buffer.trim();
                        if let (Some(question_id), false) = (&element_id, content.is_empty()) {
                            answers.entry(question_id.clone()).or_default().push(
                                CorrectionAnswer {
                                    id: "1".to_owned(),
                                    score: 0.0,
                                    content: content.to_owned(),
                                },
                            );
                        }
                    }
                    "answer" => {
                        collecting = false;
                        if let Some(answer) = current.as_mut() {
                            answer.content = buffer.clone();
                        }
                        if let (Some(question_id), Some(answer)) = (&element_id, current.take()) {
                            answers.entry(question_id.clone()).or_default().push(
                                CorrectionAnswer {
                                    id: answer.id,
                                    score: answer.score,
                                    content: answer.content.trim().to_owned(),
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if collecting {
                    buffer.push_str(&text);
                } else if group_collecting {
                    group_buffer.push_str(&text);
                }
            }
            Event::CData(text) => {
                if collecting {
                    buffer.push_str(&text);
                } else if group_collecting {
                    group_buffer.push_str(&text);
                }
            }
            _ => {}
        }
    }
    answers
}

fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a str {
    start.name().local_name().into_inner()
}

fn attribute(start: &BytesStart<'_>, name: &str) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().into_inner() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}

/// Flatten the HTML stored as a stem into one line.
fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

fn text_or_empty<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(flex::opt_string(deserializer)?.unwrap_or_default())
}

fn number_or_zero<'de, D>(deserializer: D) -> std::result::Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(flex::opt_f64(deserializer)?.unwrap_or_default())
}

fn number_or_zero_i64<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(flex::opt_i64(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    const PAPER: &str = r#"<elements>
      <element id="Q1" type="3">
        <question_text><![CDATA[<p>People make {{1}} (assume) about you.</p>]]></question_text>
        <question_no>10</question_no>
        <question_score>1</question_score>
        <question_type>4</question_type>
        <qlib_qst_type_name>填空题</qlib_qst_type_name>
      </element>
      <element id="Q2" type="3">
        <question_text><![CDATA[Choose the best answer.]]></question_text>
        <question_no>11</question_no>
        <question_score>2</question_score>
        <question_type>1</question_type>
        <qlib_qst_type_name>单项选择</qlib_qst_type_name>
        <options>
          <option id="A"><![CDATA[第一项]]></option>
          <option id="B"><![CDATA[第二项]]></option>
          <option id="C"><![CDATA[第三项]]></option>
        </options>
      </element>
    </elements>"#;

    const ANSWERS: &str = r#"<elements>
      <element id="Q1"><analysis/><answers>
        <answer id="1" score="1"><![CDATA[assumptions]]></answer>
      </answers></element>
      <element id="Q2"><analysis/><answers>AB</answers></element>
    </elements>"#;

    fn paper() -> CorrectionPaper {
        let object = json!({
            "batchId": "B1",
            "pathPrefix": "https://cdn.test",
            "paperData": PAPER,
            "answerData": ANSWERS,
        });
        correction_from_object(object.as_object().unwrap().clone(), "draw").unwrap()
    }

    #[test]
    fn the_paper_and_answer_documents_merge_into_questions() {
        let paper = paper();
        assert_eq!(paper.batch_id, "B1");
        assert_eq!(paper.questions.len(), 2);
        assert_eq!(paper.questions[0].question_id, "Q1");
        assert_eq!(paper.questions[0].number, "10");
        assert_eq!(paper.questions[0].question_type, "4");
        assert_eq!(paper.questions[0].type_name, "填空题");
        assert_eq!(paper.questions[0].score, 1.0);
        assert_eq!(
            paper.questions[0].stem,
            "People make {{1}} (assume) about you."
        );
        assert_eq!(paper.questions[0].answers[0].content, "assumptions");
        assert_eq!(paper.total_score(), 3.0);
        assert_eq!(paper.questions[1].options.len(), 3);
        assert_eq!(paper.answer_text("Q2"), "A. 第一项；B. 第二项");
    }

    #[test]
    fn qst_json_holds_one_parent_and_one_row_per_blank() {
        let paper = paper();
        let rows = qst_json(&paper);
        let rows = rows.as_array().unwrap();
        // Q1 is fill-blank (parent + one blank); Q2 is a choice (parent only).
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["elementId"], "Q1");
        assert_eq!(rows[0]["elementType"], 1);
        assert_eq!(rows[0]["elementAttr"]["question_type"], 4);
        assert_eq!(rows[1]["elementId"], "Q1-1");
        assert_eq!(rows[1]["elementType"], 2);
        assert_eq!(rows[1]["elementAttr"]["user_answer"], "assumptions");
        assert_eq!(rows[1]["elementAttr"]["result"], 1);
        assert_eq!(rows[2]["elementId"], "Q2");
        assert_eq!(rows[2]["elementAttr"]["user_answer"], "AB");
    }

    #[test]
    fn fill_blank_marks_each_blank_and_choice_marks_the_parent() {
        let xml = mark_xml(&paper());
        // Q1 is fill-blank: its blank carries the mark and the answer text.
        assert!(xml.contains("<element id=\"Q1\"><mark_data id=\"1\"><score>1</score>"));
        assert!(xml.contains("<user_answer><![CDATA[assumptions]]></user_answer>"));
        // Q2 is a choice: one parent mark worth 2 points, no answer text.
        assert!(xml.contains("<element id=\"Q2\"><mark_data id=\"1\"><score>2</score>"));
        assert!(!xml.contains(
            "<element id=\"Q2\"><mark_data id=\"1\"><score>2</score></mark_data><mark_data"
        ));
    }

    #[test]
    fn tasks_json_matches_the_client_branch() {
        let paper = paper();
        let context = CorrectionContext {
            uid: "42".to_owned(),
            client_id: "CID".to_owned(),
            course_id: -1,
            study_task_id: String::new(),
        };
        let tasks = build_tasks_json(&paper, &context, 12, 1_700_000_000_000, "PRODUCT");
        assert_eq!(tasks["uid"], "42");
        assert_eq!(tasks["batchId"], "B1");
        assert_eq!(tasks["testId"], "B1");
        assert_eq!(tasks["courseId"], -1);
        assert_eq!(tasks["seconds"], 12);
        assert_eq!(tasks["score"], 3.0);
        assert_eq!(tasks["extendParams"]["batchId"], "B1");
        assert_eq!(tasks["extendParams"]["xmlResult"]["answerdata"], ANSWERS);
        assert!(tasks["extendParams"]["xmlResult"]["markdatao"]
            .as_str()
            .unwrap()
            .starts_with("<elements>"));
        assert_eq!(tasks["qstJson"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn the_submitted_material_is_a_one_element_array() {
        let paper = paper();
        let context = CorrectionContext {
            uid: "42".to_owned(),
            ..Default::default()
        };
        let tasks = build_tasks_json(&paper, &context, 0, 0, "");
        let tasks_json = serde_json::to_string(&json!([tasks])).unwrap();
        let parsed: Value = serde_json::from_str(&tasks_json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["uid"], "42");
        assert_eq!(parsed[0]["batchId"], "B1");
    }

    #[tokio::test]
    async fn overview_decodes_the_group_columns() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/personalmistake/overview")
                .body_includes("\"studentId\":42")
                .body_includes("\"organId\":3105")
                .body_includes("\"stageId\":3");
            then.status(200).json_body(ok(json!({
                "remainingCount": 257,
                "correctedCount": 482,
                "todayCorrectedCount": 3,
                "groupList": [{
                    "groupKey": "v2.group",
                    "remainingCount": 257,
                    "questionTypeName": "填空题",
                    "knowledgePointName": "－"
                }]
            })));
        });

        let overview = correction_overview(&client(&server), 42, 3105, 3)
            .await
            .unwrap();
        assert_eq!(overview.remaining_count, 257);
        assert_eq!(overview.corrected_count, 482);
        assert_eq!(overview.group_list[0].question_type_name, "填空题");
        assert_eq!(overview.group_list[0].knowledge_point_name, "－");
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn draw_returns_none_when_the_server_has_nothing_left() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/personalmistake/correction/draw")
                .body_includes("studentId=42")
                .body_includes("questionCount=5")
                .body_includes("groupKey=G1");
            then.status(200).json_body(ok(json!({})));
        });

        let result = correction_draw(&client(&server), 42, &[], 5, Some("G1"))
            .await
            .unwrap();
        assert!(result.is_none());
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn status_decodes_the_marking_branch() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/personalmistake/correction/status")
                .body_includes("batchId=B1")
                .body_includes("uid=42");
            then.status(200).json_body(ok(json!({
                "status": 1,
                "percent": 50,
                "submitDate": 123
            })));
        });

        let status = correction_status(&client(&server), "B1", "42")
            .await
            .unwrap();
        assert_eq!(status.status, 1);
        assert_eq!(status.percent, 50.0);
        assert_eq!(status.submit_date, 123);
        mock.assert_calls(1);
    }

    #[test]
    fn a_paper_without_questions_is_a_decode_error() {
        let object = json!({
            "batchId": "B1",
            "paperData": "<elements/>",
            "answerData": ""
        });
        assert!(correction_from_object(object.as_object().unwrap().clone(), "draw").is_err());
    }
}
