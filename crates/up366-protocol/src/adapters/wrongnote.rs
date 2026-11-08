//! Wrong-answer book adapters: reads, the kill paper, and the cleanup write.
//!
//! Read/write boundary is the important part here:
//!
//! - `questions/kill` is a **read**: it returns the paper the student must
//!   answer to clear wrong questions.
//! - `question/submit` is the only destructive call.
//!
//! Cleanup is inherently batch-shaped, and the official client submits one
//! question per call. The write path is built so that:
//!
//! - a single question is the unit, and the caller decides how many to send;
//! - nothing is retried automatically — `-13`/`-21` identify a burned or dead
//!   attempt, and a timeout is a `Transport` error the caller must resolve;
//! - the per-question result is returned so a partial batch can be resumed.

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

/// One subject row from `subject/list`.
///
/// Ids arrive as numbers (`subjectId: 303`, `stageId: 3`), so they are read
/// through the tolerant deserializers and kept as text. `id` is a MongoDB
/// ObjectId the client never needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrongSubject {
    #[serde(deserialize_with = "text_or_empty")]
    pub subject_id: String,
    #[serde(default, deserialize_with = "text_or_empty")]
    pub subject_name: String,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub stage_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "number_or_zero")]
    pub wrong_count: i64,
    #[serde(default, deserialize_with = "number_or_zero")]
    pub killed_count: i64,
}

/// One knowledge-point row from `knowledge/list/v3`.
///
/// The question-type columns are spelled `qTypeId` / `qTypeName`, which
/// `rename_all` cannot produce from `qtype_id`, hence the explicit names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrongKnowledge {
    #[serde(deserialize_with = "text_or_empty")]
    pub knowledge_id: String,
    #[serde(default, deserialize_with = "text_or_empty")]
    pub knowledge_name: String,
    #[serde(default, rename = "qTypeId", deserialize_with = "text_or_empty")]
    pub qtype_id: String,
    #[serde(default, rename = "qTypeName", deserialize_with = "text_or_empty")]
    pub qtype_name: String,
    #[serde(default, deserialize_with = "number_or_zero")]
    pub wrong_count: i64,
}

/// One un-cleared question from `student/question/show`.
///
/// The row only carries ids and names: the stem and the two answers live in the
/// `paperData` / `answerData` / `userAnswerData` XML strings, so
/// [`wrong_pending`] fills them in from there.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct WrongQuestion {
    pub question_id: String,
    pub title: String,
    pub question_type: String,
    pub stem: String,
    pub correct_answer: String,
    pub student_answer: String,
}

/// The JSON row behind one question; everything else comes from the XML.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WrongQuestionRow {
    #[serde(deserialize_with = "text_or_empty")]
    question_id: String,
    #[serde(default, rename = "qTypeName", deserialize_with = "text_or_empty")]
    qtype_name: String,
    #[serde(default, deserialize_with = "text_or_empty")]
    knowledge_name: String,
    #[serde(default, deserialize_with = "text_or_empty")]
    user_answer: String,
}

/// One `question/show` page: the display rows and the XML the cleanup
/// submission is built from.
///
/// The rows are only ids and labels; `answer_xml` is the standard answer
/// document and `paper_xml` carries the per-question score used by the mark
/// document. Both are kept verbatim so the submission can copy them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingPaper {
    pub questions: Vec<WrongQuestion>,
    pub paper_xml: String,
    pub answer_xml: String,
}

/// The paper a student must answer to clear questions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillPaper {
    pub path_prefix: String,
    pub paper_xml: String,
    pub answer_xml: String,
}

/// Parameters shared by the wrong-note list endpoints.
///
/// Every field is optional here, but the list endpoints reject a call that
/// lacks `stageId`, `subjectId`, `bookId` or the time range, so callers must
/// fill them in; absent fields are simply not sent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WrongQuery {
    pub stage_id: Option<String>,
    pub subject_id: Option<String>,
    pub book_id: Option<String>,
    pub qtype_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

impl WrongQuery {
    /// The form body for one call, omitting whatever is absent.
    ///
    /// `None` fields are dropped rather than sent empty, matching the client:
    /// an empty string is a real value to the server and is not the same as
    /// "not applicable".
    fn form(&self) -> Vec<(&'static str, Value)> {
        let mut fields: Vec<(&'static str, Value)> = Vec::new();
        for (key, value) in [
            ("stageId", self.stage_id.clone()),
            ("subjectId", self.subject_id.clone()),
            ("bookId", self.book_id.clone()),
            ("qtypeId", self.qtype_id.clone()),
            ("knowledgeId", self.knowledge_id.clone()),
            ("startTime", self.start_time.clone()),
            ("endTime", self.end_time.clone()),
        ] {
            if let Some(value) = value {
                fields.push((key, json!(value)));
            }
        }
        for (key, value) in [
            ("pager.currentPage", self.page),
            ("pager.pageSize", self.page_size),
        ] {
            if let Some(value) = value {
                fields.push((key, json!(value)));
            }
        }
        fields
    }
}

/// The student's subjects with wrong-question counts.
pub async fn wrong_subjects(client: &Client) -> Result<Vec<WrongSubject>> {
    let endpoint = endpoint("wrong_subjects")?;
    let data = client.call(endpoint, &[]).await?;
    subjects_from_data(data, endpoint.name)
}

/// Knowledge points for one subject/book.
///
/// The server sorts by the column named in `orderField`; this client always
/// wants the most-wrong first, matching the H5 page.
pub async fn wrong_knowledge(client: &Client, query: &WrongQuery) -> Result<Vec<WrongKnowledge>> {
    let endpoint = endpoint("wrong_knowledge")?;
    let mut form = query.form();
    form.push(("orderType", json!("desc")));
    form.push(("orderField", json!("wrongTimes")));
    let data = client.call(endpoint, &form).await?;
    knowledge_from_data(data, endpoint.name)
}

/// The questions still to clear for one knowledge point.
pub async fn wrong_pending(client: &Client, query: &WrongQuery) -> Result<Vec<WrongQuestion>> {
    Ok(wrong_pending_paper(client, query).await?.questions)
}

/// The same page, with the paper and answer documents the cleanup needs.
pub async fn wrong_pending_paper(client: &Client, query: &WrongQuery) -> Result<PendingPaper> {
    let endpoint = endpoint("wrong_question_show")?;
    let data = client.call(endpoint, &query.form()).await?;
    pending_paper_from_data(data, endpoint.name)
}

/// Decode one `subject/list` payload: a bare array of rows.
fn subjects_from_data(data: Option<Value>, endpoint: &str) -> Result<Vec<WrongSubject>> {
    let items = codec::into_array(data, endpoint)?;
    items
        .into_iter()
        .map(|item| decode(item, endpoint))
        .collect()
}

/// Decode one `knowledge/list/v3` payload: `data.{list, pager}`.
fn knowledge_from_data(data: Option<Value>, endpoint: &str) -> Result<Vec<WrongKnowledge>> {
    let mut object = codec::into_object(data, endpoint)?;
    let items = codec::take_list(&mut object, "list", endpoint)?;
    items
        .into_iter()
        .map(|item| decode(item, endpoint))
        .collect()
}

/// Decode one `question/show` payload and keep the two paper documents.
///
/// The list rows are the skeleton; stems and answers live in the three XML
/// documents. A question the XML does not mention keeps an empty field rather
/// than failing the page, so one odd row cannot hide the rest.
fn pending_paper_from_data(data: Option<Value>, endpoint: &str) -> Result<PendingPaper> {
    let mut object = codec::into_object(data, endpoint)?;
    let items = codec::take_list(&mut object, "list", endpoint)?;
    let paper_xml = xml_field(&object, "paperData");
    let answer_xml = xml_field(&object, "answerData");
    let papers = PaperText::parse(&paper_xml);
    let answers = PaperText::parse(&answer_xml);
    let user_answers = PaperText::parse(&xml_field(&object, "userAnswerData"));

    let questions = items
        .into_iter()
        .map(|item| {
            let row: WrongQuestionRow = decode(item, endpoint)?;
            let student_answer = match user_answers.answer(&row.question_id) {
                answer if answer.is_empty() => {
                    PaperText::parse(&row.user_answer).answer(&row.question_id)
                }
                answer => answer,
            };
            Ok(WrongQuestion {
                title: title_for(&row.qtype_name, &row.knowledge_name),
                question_type: row.qtype_name,
                stem: papers.stem(&row.question_id),
                correct_answer: answers.answer(&row.question_id),
                student_answer,
                question_id: row.question_id,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(PendingPaper {
        questions,
        paper_xml,
        answer_xml,
    })
}

/// One XML-bearing field as text; a missing or non-string value is empty.
fn xml_field(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A short label for the list: the question type, plus the knowledge point
/// when the server actually names one (`""` and `"--"` are its placeholders).
fn title_for(qtype_name: &str, knowledge_name: &str) -> String {
    match knowledge_name.trim() {
        "" | "--" => qtype_name.to_owned(),
        name => format!("{qtype_name}（{name}）"),
    }
}

/// What the walker is currently collecting text for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Collecting {
    Nothing,
    Stem,
    Answer,
    Score,
}

/// Text lifted out of the wrong-note XML payloads, keyed by question id.
///
/// `paperData` keeps the stem in `<question_text>`; `answerData`,
/// `userAnswerData` and a row's `userAnswer` keep the answers as a list of
/// `<answer>` texts.
#[derive(Debug, Default)]
struct PaperText {
    stems: HashMap<String, String>,
    answers: HashMap<String, Vec<String>>,
    scores: HashMap<String, String>,
}

impl PaperText {
    /// Walk one document. Malformed XML keeps whatever was read before the
    /// error, so a bad payload degrades to empty fields instead of a dead page.
    fn parse(xml: &str) -> Self {
        let mut parsed = Self::default();
        if xml.trim().is_empty() {
            return parsed;
        }

        let mut reader = Reader::from_str(xml);
        let mut question_id: Option<String> = None;
        let mut collecting = Collecting::Nothing;
        let mut buffer = String::new();

        loop {
            let event = match reader.read_event() {
                Ok(Event::Eof) | Err(_) => break,
                Ok(event) => event,
            };
            match event {
                Event::Start(start) => match local_name(&start) {
                    "element" => question_id = attribute(&start, "id"),
                    "question_text" => {
                        collecting = Collecting::Stem;
                        buffer.clear();
                    }
                    "answer" => {
                        collecting = Collecting::Answer;
                        buffer.clear();
                    }
                    "question_score" => {
                        collecting = Collecting::Score;
                        buffer.clear();
                    }
                    _ => {}
                },
                Event::End(end) => match end.name().local_name().into_inner() {
                    "element" => {
                        question_id = None;
                        collecting = Collecting::Nothing;
                    }
                    "question_text" => {
                        if let Some(id) = &question_id {
                            parsed.stems.insert(id.clone(), plain_text(&buffer));
                        }
                        collecting = Collecting::Nothing;
                    }
                    "answer" => {
                        if let Some(id) = &question_id {
                            let answer = buffer.trim();
                            if !answer.is_empty() {
                                parsed
                                    .answers
                                    .entry(id.clone())
                                    .or_default()
                                    .push(answer.to_owned());
                            }
                        }
                        collecting = Collecting::Nothing;
                    }
                    "question_score" => {
                        if let Some(id) = &question_id {
                            let score = buffer.trim();
                            if !score.is_empty() {
                                parsed.scores.insert(id.clone(), score.to_owned());
                            }
                        }
                        collecting = Collecting::Nothing;
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if collecting != Collecting::Nothing {
                        buffer.push_str(&text);
                    }
                }
                Event::CData(text) => {
                    if collecting != Collecting::Nothing {
                        buffer.push_str(&text);
                    }
                }
                _ => {}
            }
        }
        parsed
    }

    fn stem(&self, question_id: &str) -> String {
        self.stems.get(question_id).cloned().unwrap_or_default()
    }

    /// Every blank's answer, in document order.
    fn answer(&self, question_id: &str) -> String {
        self.answers
            .get(question_id)
            .map(|parts| parts.join("；"))
            .unwrap_or_default()
    }

    /// The full score of one question, as the paper declares it.
    fn score(&self, question_id: &str) -> Option<&str> {
        self.scores.get(question_id).map(String::as_str)
    }
}

/// The local tag name, without any namespace prefix.
fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a str {
    start.name().local_name().into_inner()
}

/// One attribute of an element, when it is present and decodable.
fn attribute(start: &BytesStart<'_>, name: &str) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().into_inner() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}

/// Flatten the HTML the paper stores as a stem into the single line the list
/// renders: tags dropped, entities decoded, whitespace collapsed.
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

/// Text that may arrive as a number; absent or `null` becomes empty.
fn text_or_empty<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(flex::opt_string(deserializer)?.unwrap_or_default())
}

/// Read a count as an integer, accepting a numeric string; anything else is 0.
fn number_or_zero<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(flex::opt_i64(deserializer)?.unwrap_or_default())
}

/// The paper to answer in order to clear questions.
pub async fn wrong_kill_paper(client: &Client, query: &WrongQuery) -> Result<KillPaper> {
    let endpoint = endpoint("wrong_kill_paper")?;
    let data = client.call(endpoint, &query.form()).await?;
    let object = codec::into_object(data, endpoint.name)?;
    Ok(KillPaper {
        path_prefix: string_field(&object, "pathPrefix")?,
        paper_xml: string_field(&object, "paperData")?,
        answer_xml: string_field(&object, "answerData")?,
    })
}

/// One question's answer, as the kill paper describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongAnswer {
    pub question_id: String,
    /// `answerdata`: one `<elements>` document holding this question's answer.
    pub answer_xml: String,
    /// `mark_datao`: one `<elements>` document marking the question correct.
    pub mark_xml: String,
}

impl WrongAnswer {
    /// Build one submission from the standard answer document.
    ///
    /// The answer element is copied verbatim, so every question type keeps the
    /// exact shape the server sent. A question the document does not mention
    /// returns `None`: sending an empty answer would burn the attempt.
    pub fn from_documents(question_id: &str, paper_xml: &str, answer_xml: &str) -> Option<Self> {
        let element = element_xml(answer_xml, question_id)?;
        let score = PaperText::parse(paper_xml)
            .score(question_id)
            .unwrap_or("0")
            .to_owned();
        Some(Self {
            question_id: question_id.to_owned(),
            answer_xml: format!("<elements>{element}</elements>"),
            mark_xml: mark_xml(question_id, &score),
        })
    }
}

/// One `<element id="...">...</element>` block, byte for byte.
///
/// The answer document is server-authored XML; slicing the original keeps
/// CDATA, attributes and question-type-specific children intact.
fn element_xml(document: &str, question_id: &str) -> Option<String> {
    if document.trim().is_empty() {
        return None;
    }

    let mut reader = Reader::from_str(document);
    let mut depth = 0usize;
    let mut start = 0usize;
    loop {
        let event_start = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        match event {
            Event::Eof => return None,
            Event::Start(tag) => {
                if depth == 0 {
                    if local_name(&tag) == "element"
                        && attribute(&tag, "id").as_deref() == Some(question_id)
                    {
                        depth = 1;
                        start = event_start;
                    }
                } else {
                    depth += 1;
                }
            }
            // A self-closing element carries no answer: treat it as absent.
            Event::Empty(tag) => {
                if depth == 0
                    && local_name(&tag) == "element"
                    && attribute(&tag, "id").as_deref() == Some(question_id)
                {
                    return None;
                }
            }
            Event::End(_) => {
                if depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        let end = reader.buffer_position() as usize;
                        return Some(document[start..end].to_owned());
                    }
                }
            }
            _ => {}
        }
    }
}

/// The mark document for one correctly answered question.
fn mark_xml(question_id: &str, score: &str) -> String {
    let score = if score.trim().is_empty() {
        "0"
    } else {
        score.trim()
    };
    format!(
        "<elements><element id=\"{}\"><mark_data><score>{}</score><result>1</result></mark_data></element></elements>",
        quick_xml::escape::escape(question_id),
        quick_xml::escape::escape(score)
    )
}

/// The payload of one cleanup submission.
///
/// Mirrors the client's six fields: `uid`, `result` (JSON of the two XML
/// fragments), the server-measured time range, `seconds`, and `rightQstIds`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongCleanup {
    pub uid: String,
    pub answer_xml: String,
    pub mark_xml: String,
    pub start_time: i64,
    pub end_time: i64,
    pub right_qst_ids: String,
}

impl WrongCleanup {
    /// The `result` field: a JSON string holding both XML fragments.
    pub fn result_json(&self) -> String {
        let payload = json!({
            "answerdata": self.answer_xml,
            "mark_datao": self.mark_xml,
        });
        serde_json::to_string(&payload).expect("payload is serializable")
    }

    /// The full form body of one submission.
    pub fn form(&self) -> Vec<(&'static str, Value)> {
        // `seconds` is derived, never taken from the caller, so a UI bug
        // cannot send an inconsistent time range.
        let seconds = ((self.end_time - self.start_time).max(0) + 999) / 1000;
        vec![
            ("uid", json!(&self.uid)),
            ("result", json!(self.result_json())),
            ("startTime", json!(self.start_time)),
            ("endTime", json!(self.end_time)),
            ("seconds", json!(seconds)),
            ("rightQstIds", json!(&self.right_qst_ids)),
        ]
    }
}

/// Submit one question's cleanup.
///
/// A real write: the caller must have shown a preview and confirmed.
pub async fn submit_wrong_question(client: &Client, cleanup: &WrongCleanup) -> Result<Value> {
    let endpoint = endpoint("submit_wrong_question")?;
    if cleanup.uid.is_empty() {
        return Err(ProtocolError::Invalid("uid is required".to_owned()));
    }
    let data = client.call(endpoint, &cleanup.form()).await?;
    Ok(data.unwrap_or(Value::Null))
}

/// Look up an endpoint by name, with a precise error if the table is wrong.
fn endpoint(name: &str) -> Result<&'static crate::endpoint::Endpoint> {
    get(name).ok_or_else(|| ProtocolError::Decode(format!("missing endpoint: {name}")))
}

/// Decode one item of a list response.
fn decode<T>(item: Value, endpoint: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(item)
        .map_err(|error| ProtocolError::Decode(format!("{endpoint}: {error}")))
}

/// Read a required string field from a response object.
fn string_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| ProtocolError::Decode(format!("missing field: {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    /// The real shape of one `subject/list` row: numeric ids, camelCase.
    #[test]
    fn subject_rows_are_camel_case_and_accept_numeric_ids() {
        let data = json!([{
            "bookId": "0",
            "id": "{\"date\":1794038586000}",
            "killedCount": 0,
            "stageId": 3,
            "subjectId": 303,
            "subjectName": "高中英语",
            "wrongCount": 413
        }]);
        let subjects = subjects_from_data(Some(data), "wrong_subjects").unwrap();
        assert_eq!(subjects.len(), 1);
        assert_eq!(subjects[0].subject_id, "303");
        assert_eq!(subjects[0].subject_name, "高中英语");
        assert_eq!(subjects[0].stage_id.as_deref(), Some("3"));
        assert_eq!(subjects[0].book_id.as_deref(), Some("0"));
        assert_eq!(subjects[0].wrong_count, 413);
        assert_eq!(subjects[0].killed_count, 0);
    }

    #[tokio::test]
    async fn knowledge_rows_decode_qtype_from_q_type_id() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/wrongnote/student/knowledge/list/v3")
                .body_contains("stageId=3")
                .body_contains("subjectId=303")
                .body_contains("bookId=0")
                .body_contains("startTime=2010-01-01+00%3A00%3A00")
                .body_contains("endTime=2026-11-08+23%3A59%3A59")
                .body_contains("orderType=desc")
                .body_contains("orderField=wrongTimes")
                .body_contains("pager.pageSize=100");
            then.status(200).json_body(ok(json!({
                "list": [{
                    "knowledgeId": "046001",
                    "knowledgeName": "--",
                    "qTypeId": 448,
                    "qTypeName": "语法填空",
                    "wrongCount": 19
                }],
                "pager": { "currentPage": 1, "pageSize": 100, "totalCount": 1 }
            })));
        });

        let query = WrongQuery {
            stage_id: Some("3".to_owned()),
            subject_id: Some("303".to_owned()),
            book_id: Some("0".to_owned()),
            page: Some(1),
            page_size: Some(100),
            start_time: Some("2010-01-01 00:00:00".to_owned()),
            end_time: Some("2026-11-08 23:59:59".to_owned()),
            ..Default::default()
        };
        let rows = wrong_knowledge(&client(&server), &query).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].knowledge_id, "046001");
        assert_eq!(rows[0].qtype_id, "448");
        assert_eq!(rows[0].qtype_name, "语法填空");
        assert_eq!(rows[0].wrong_count, 19);
        mock.assert_hits(1);
    }

    /// A synthetic paper: one stem, one correct answer, one student answer.
    #[tokio::test]
    async fn pending_merges_the_paper_xml_into_the_list_rows() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/wrongnote/student/question/show")
                .body_contains("stageId=3")
                .body_contains("subjectId=303")
                .body_contains("bookId=0")
                .body_contains("qtypeId=448")
                .body_contains("knowledgeId=046001")
                .body_contains("startTime=2010-01-01+00%3A00%3A00")
                .body_contains("pager.currentPage=1")
                .body_contains("pager.pageSize=10");
            then.status(200).json_body(ok(json!({
                "pathPrefix": "https://cdn.test",
                "paperData": "<elements><element type=\"3\" id=\"Q1\"><question_no>1</question_no><question_text><![CDATA[<p>1 + 1 = {{1}}</p>]]></question_text></element></elements>",
                "answerData": "<elements><element id=\"Q1\"><answers><answer id=\"1\"><![CDATA[2]]></answer><answer id=\"2\"><![CDATA[two]]></answer></answers></element></elements>",
                "userAnswerData": "<elements><element id=\"Q1\"><answers><answer id=\"1\"><![CDATA[3]]></answer><answer id=\"2\"><![CDATA[three]]></answer></answers></element></elements>",
                "list": [
                    { "questionId": "Q1", "qTypeId": 448, "qTypeName": "语法填空", "knowledgeName": "--" },
                    { "questionId": "Q2", "qTypeId": 448, "qTypeName": "语法填空", "knowledgeName": "时态" }
                ],
                "pager": { "currentPage": 1, "pageSize": 10 }
            })));
        });

        let query = WrongQuery {
            stage_id: Some("3".to_owned()),
            subject_id: Some("303".to_owned()),
            book_id: Some("0".to_owned()),
            qtype_id: Some("448".to_owned()),
            knowledge_id: Some("046001".to_owned()),
            page: Some(1),
            page_size: Some(10),
            start_time: Some("2010-01-01 00:00:00".to_owned()),
            end_time: Some("2026-11-08 23:59:59".to_owned()),
        };
        let rows = wrong_pending(&client(&server), &query).await.unwrap();

        assert_eq!(rows.len(), 2);
        // Q1 is in the paper: stem and both answers come from the XML.
        assert_eq!(rows[0].question_id, "Q1");
        assert_eq!(rows[0].title, "语法填空");
        assert_eq!(rows[0].question_type, "语法填空");
        assert_eq!(rows[0].stem, "1 + 1 = {{1}}");
        assert_eq!(rows[0].correct_answer, "2；two");
        assert_eq!(rows[0].student_answer, "3；three");
        // Q2 has no row in the XML: the id survives, the rest stays empty.
        assert_eq!(rows[1].question_id, "Q2");
        assert_eq!(rows[1].title, "语法填空（时态）");
        assert_eq!(rows[1].stem, "");
        assert_eq!(rows[1].correct_answer, "");
        mock.assert_hits(1);
    }

    #[test]
    fn a_broken_paper_degrades_to_empty_fields_instead_of_failing() {
        let data = json!({
            "paperData": "<elements><element id=\"Q1\"><question_text><![CDATA[half",
            "answerData": "not xml at all <<<",
            "list": [{ "questionId": "Q1", "qTypeName": "选择题" }]
        });
        let rows = pending_paper_from_data(Some(data), "wrong_question_show")
            .unwrap()
            .questions;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].question_id, "Q1");
        assert_eq!(rows[0].stem, "");
        assert_eq!(rows[0].correct_answer, "");
    }

    #[test]
    fn the_student_answer_falls_back_to_the_row_fragment() {
        let data = json!({
            "paperData": "<elements><element id=\"Q1\"><question_text><![CDATA[题干]]></question_text></element></elements>",
            "list": [{
                "questionId": "Q1",
                "qTypeName": "选择题",
                "userAnswer": "<element id=\"Q1\"><answers><answer id=\"1\"><![CDATA[B]]></answer></answers></element>"
            }]
        });
        let rows = pending_paper_from_data(Some(data), "wrong_question_show")
            .unwrap()
            .questions;
        assert_eq!(rows[0].stem, "题干");
        assert_eq!(rows[0].student_answer, "B");
    }

    #[test]
    fn cleanup_form_carries_the_six_fields_and_derives_seconds() {
        let cleanup = WrongCleanup {
            uid: "42".to_owned(),
            answer_xml: "<answerdata/>".to_owned(),
            mark_xml: "<elements/>".to_owned(),
            start_time: 1_000,
            end_time: 4_500,
            right_qst_ids: "Q1,Q2".to_owned(),
        };
        let fields = cleanup.form();
        let names: Vec<_> = fields.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            vec![
                "uid",
                "result",
                "startTime",
                "endTime",
                "seconds",
                "rightQstIds"
            ]
        );

        let seconds = fields
            .iter()
            .find(|(name, _)| *name == "seconds")
            .map(|(_, value)| value.as_i64().unwrap());
        // ceil((4500-1000)/1000) = 4, not the caller's 99.
        assert_eq!(seconds, Some(4));
    }

    #[test]
    fn result_json_holds_both_xml_fragments() {
        let cleanup = WrongCleanup {
            uid: "1".to_owned(),
            answer_xml: "<a/>".to_owned(),
            mark_xml: "<m/>".to_owned(),
            start_time: 0,
            end_time: 1,
            right_qst_ids: String::new(),
        };
        let parsed: Value = serde_json::from_str(&cleanup.result_json()).unwrap();
        assert_eq!(parsed["answerdata"], "<a/>");
        assert_eq!(parsed["mark_datao"], "<m/>");
    }

    #[test]
    fn absent_optional_fields_are_dropped_from_the_form() {
        let query = WrongQuery {
            subject_id: Some("2".to_owned()),
            ..Default::default()
        };
        let fields = query.form();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "subjectId");
    }

    #[test]
    fn a_submission_without_a_uid_is_rejected_before_any_request() {
        let cleanup = WrongCleanup {
            uid: String::new(),
            answer_xml: String::new(),
            mark_xml: String::new(),
            start_time: 0,
            end_time: 0,
            right_qst_ids: String::new(),
        };
        // No server is needed: the guard fires before the transport.
        assert!(matches!(
            tokio_test(submit_wrong_question(&Client::new().unwrap(), &cleanup)),
            Err(ProtocolError::Invalid(_))
        ));
    }

    #[test]
    fn an_answer_element_is_copied_verbatim_for_the_selected_question() {
        let answer_xml = r#"<elements><element id="Q1"><answers><answer id="1"><![CDATA[2]]></answer></answers></element><element id="Q2"><answers><answer id="1"><![CDATA[B]]></answer></answers></element></elements>"#;
        let paper_xml = r#"<elements><element id="Q1"><question_score>5.0</question_score></element><element id="Q2"><question_score>10</question_score></element></elements>"#;

        let answer = WrongAnswer::from_documents("Q2", paper_xml, answer_xml).unwrap();

        assert_eq!(answer.question_id, "Q2");
        assert_eq!(
            answer.answer_xml,
            r#"<elements><element id="Q2"><answers><answer id="1"><![CDATA[B]]></answer></answers></element></elements>"#
        );
        assert_eq!(
            answer.mark_xml,
            r#"<elements><element id="Q2"><mark_data><score>10</score><result>1</result></mark_data></element></elements>"#
        );
    }

    #[test]
    fn a_question_without_an_answer_element_is_skipped() {
        let answer_xml = r#"<elements><element id="Q1"><answers><answer id="1"><![CDATA[2]]></answer></answers></element></elements>"#;
        assert!(WrongAnswer::from_documents("Q2", "", answer_xml).is_none());
    }

    #[test]
    fn a_self_closing_answer_element_is_skipped() {
        let answer_xml = r#"<elements><element id="Q1"/></elements>"#;
        assert!(WrongAnswer::from_documents("Q1", "", answer_xml).is_none());
    }

    fn tokio_test<F>(future: F) -> Result<Value>
    where
        F: std::future::Future<Output = Result<Value>>,
    {
        tokio::runtime::Runtime::new().unwrap().block_on(future)
    }
}
