//! Read-aloud score payload.
//!
//! The scheduled-job builder is confirmed against two live submissions (see
//! `docs/protocol/confirmed.md`):
//!
//! - one `<answer>` per question in `result.testInfo.answerdata`, carrying the
//!   uploaded URL, the reported score and the question text;
//! - `qstJson` keeps each question's own `question_type` (9/12/13), because the
//!   recording branch serves spoken follow-read, Q&A and retelling alike;
//! - one `<mark_data>` per question in `result.testInfo.markdatao`;
//! - `qstJson` is a **flat array**: one `elementType = 1` row per question and
//!   one `elementType = 2` row per sentence (`{questionId}-{n}`);
//! - `studyTaskId` must equal the job id, and the request is
//!   `POST /client/task/score/submit/v2` with `submitType = 8`.
//!
//! Course-book pages can use the separate legacy flipbook builder below:
//! `srcType = 1`, empty XML `result`, and stringified `elementAttr` rows in
//! `qstJson`. The official reader obtains these from local `TaskStuded`
//! persistence; this builder emits the flattened upload rows directly.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::paper::{PaperQuestion, ReadAloudPaper};
use crate::client::Client;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};
use crate::signature::submit_task_ut;

/// `submitType` the textbook read-aloud branch uses for a scheduled job.
pub const READALOUD_SUBMIT_TYPE: i64 = 8;
/// `submitType` the course-book reader uses for a page with no job. The reader
/// derives it as `studyTaskId ? 4 : 0`, and a class-book page has no job id.
pub const BOOK_TASK_SUBMIT_TYPE: i64 = 0;
const DEVICE_TYPE: &str = "Win10";

/// One uploaded recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadAloudRecording {
    pub question_id: String,
    /// OSS URL returned by [`super::upload_object`]'s caller.
    pub record_url: String,
    /// Duration of the uploaded recording. Older stored assets may not have
    /// this metadata, in which case the paper's reference duration is used.
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

struct ScoredRecording {
    result: i64,
    duration_seconds: u32,
    element_attr: Value,
}

fn score_recording(
    question: &PaperQuestion,
    recording: &ReadAloudRecording,
    score: f64,
    study_date_ms: i64,
) -> Result<ScoredRecording> {
    if !(0.0..=question.score).contains(&score) {
        return Err(ProtocolError::Invalid(format!(
            "score {score} is outside 0..={} for question {}",
            question.score, question.question_id
        )));
    }
    let result = if score >= question.score {
        1
    } else if score == 0.0 {
        2
    } else {
        3
    };
    let duration_seconds = recording
        .duration_ms
        .map(|duration| duration.div_ceil(1000).min(u64::from(u32::MAX)) as u32)
        .unwrap_or(question.duration);
    let element_attr = json!({
        "question_id": question.question_id,
        "question_type": question.question_type,
        "order": 1,
        "user_score": score,
        "user_answer": "",
        "answer": "",
        "score": question.score,
        "result": result,
        "timestamp": study_date_ms,
        "record_url": recording.record_url,
        "duration": duration_seconds,
        "wrongnote_flag": 1,
        "recForm": 1,
        "comment": "",
        "comment_ai": "",
        "pic_url": "",
        "ocrTids": "",
        "record_result": "",
        "accuracy": 0,
        "completion": 0,
        "fluency": 0,
        "code": 0,
    });

    Ok(ScoredRecording {
        result,
        duration_seconds,
        element_attr,
    })
}

struct QstElement {
    element_id: String,
    element_type: i64,
    order: usize,
    element_attr: Value,
}

fn qst_elements(question_id: &str, sentence_count: usize, attr: &Value) -> Vec<QstElement> {
    let mut elements = Vec::with_capacity(sentence_count + 1);
    elements.push(QstElement {
        element_id: question_id.to_owned(),
        element_type: 1,
        order: 1,
        element_attr: attr.clone(),
    });
    for order in 1..=sentence_count {
        let element_id = format!("{question_id}-{order}");
        let mut element_attr = attr.clone();
        element_attr["question_id"] = json!(&element_id);
        element_attr["order"] = json!(order);
        elements.push(QstElement {
            element_id,
            element_type: 2,
            order,
            element_attr,
        });
    }
    elements
}

fn score_percent(total: f64, maximum: f64) -> f64 {
    if maximum > 0.0 {
        (total / maximum * 100.0 * 100.0).round() / 100.0
    } else {
        0.0
    }
}

/// Ids every read-aloud submission needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAloudContext {
    pub uid: i64,
    pub client_id: String,
    pub course_id: String,
    pub book_id: String,
    /// `taskId` / content page id used by the score endpoint.
    pub content_id: String,
    /// The containing chapter id. Scheduled jobs often use the content id;
    /// course-book pages use the page's `parentId` from the book tree.
    pub chapter_id: String,
    /// The scheduled job id. `None` for a course-book page; the caller then
    /// selects the reader-specific row shape and keeps `studyTaskId` empty.
    pub job_id: Option<String>,
    pub paper_id: String,
}

/// Build one `tasksJson` row without sending it.
///
/// `scores` are per question and must stay within `0..=question.score`.
/// `study_date_ms` is the submission timestamp. `seconds` is the study time the
/// client reports; the transport does not derive it.
pub fn build_row(
    paper: &ReadAloudPaper,
    context: &ReadAloudContext,
    recordings: &[ReadAloudRecording],
    scores: &[f64],
    study_date_ms: i64,
    seconds: i64,
) -> Result<Value> {
    if recordings.len() != paper.questions.len() {
        return Err(ProtocolError::Invalid(format!(
            "expected {} recording(s), got {}",
            paper.questions.len(),
            recordings.len()
        )));
    }
    if scores.len() != paper.questions.len() {
        return Err(ProtocolError::Invalid(format!(
            "expected {} score(s), got {}",
            paper.questions.len(),
            scores.len()
        )));
    }

    let mut answer_elements = Vec::with_capacity(paper.questions.len());
    let mut mark_elements = Vec::with_capacity(paper.questions.len());
    let mut qst_json = Vec::with_capacity(
        paper
            .questions
            .iter()
            .map(|question| question.sentences.len() + 1)
            .sum(),
    );
    let mut total = 0.0;

    for (index, question) in paper.questions.iter().enumerate() {
        let score = scores[index];
        if !(0.0..=question.score).contains(&score) {
            return Err(ProtocolError::Invalid(format!(
                "score {score} is outside 0..={} for question {}",
                question.score, question.question_id
            )));
        }
        let recording = recordings
            .iter()
            .find(|recording| recording.question_id == question.question_id)
            .ok_or_else(|| {
                ProtocolError::Invalid(format!(
                    "no recording for question {}",
                    question.question_id
                ))
            })?;
        let scored = score_recording(question, recording, score, study_date_ms)?;
        let result = scored.result;
        total += score;

        let text = question.sentences.join(" ");
        let question_id = question.question_id.as_str();
        let record_url = recording.record_url.as_str();
        let duration = scored.duration_seconds;
        answer_elements.push(format!(
            "<element id=\"{id}\"><answers>\
             <answer_url>{url}</answer_url>\
             <answer_score>{score}</answer_score>\
             <answer_rejectscore>0</answer_rejectscore>\
             <answer_text><![CDATA[{text}]]></answer_text>\
             <answer_recordmin><![CDATA[0]]></answer_recordmin>\
             <answer_recordsec><![CDATA[{duration}]]></answer_recordsec>\
             </answers></element>",
            id = question_id,
            url = record_url,
            score = score_text(score),
            text = cdata(&text),
            duration = duration,
        ));
        mark_elements.push(format!(
            "<element id=\"{id}\"><mark_data><score>{score}</score>\
             <result>{result}</result></mark_data></element>",
            id = question_id,
            score = score_text(score),
        ));

        for element in qst_elements(question_id, question.sentences.len(), &scored.element_attr) {
            qst_json.push(json!({
                "elementId": element.element_id,
                "elementType": element.element_type,
                "elementAttr": element.element_attr,
            }));
        }
    }

    let answerdata = format!(
        "<elements>{}</elements>",
        answer_elements.into_iter().collect::<String>()
    );
    let markdatao = format!(
        "<elements>{}</elements>",
        mark_elements.into_iter().collect::<String>()
    );
    let result = json!({
        "studentid": context.uid,
        "testInfo": { "answerdata": answerdata, "markdatao": markdatao },
    })
    .to_string();
    let percent = score_percent(total, paper.total_score);

    // The two submission sources carry different rows. A scheduled job sends
    // its `studyTaskId`, the paper id, the per-question `qstJson` and the
    // device fields (verified live). A legacy packType<7 reader uses
    // `build_legacy_reader_row`; this branch is the XML textbook bridge.
    let job_id = context.job_id.as_deref();
    let mut row = json!({
        "id": uuid::Uuid::new_v4().simple().to_string(),
        "taskId": context.content_id,
        "uid": context.uid,
        "bookId": context.book_id,
        "chapterId": context.chapter_id,
        "taskNo": 1,
        "score": total,
        "percent": percent,
        "studyDate": study_date_ms,
        "seconds": seconds,
        "result": result,
        "srcType": 2,
        "batchTag": uuid::Uuid::new_v4().simple().to_string(),
        "uploaded": 0,
        "msg": "",
        "studyTaskId": job_id.unwrap_or_default(),
        "courseId": context.course_id,
        "extendParams": { "batchId": uuid::Uuid::new_v4().simple().to_string() },
        "failed": 0,
        "killStudyTask": 0,
        "submitType": if job_id.is_some() {
            READALOUD_SUBMIT_TYPE
        } else {
            BOOK_TASK_SUBMIT_TYPE
        },
    });
    if job_id.is_some() {
        row["paperId"] = json!(context.paper_id);
        row["qstJson"] = json!(qst_json);
        row["deviceType"] = json!(DEVICE_TYPE);
        row["clientId"] = json!(context.client_id);
        row["wrongnoteFlag"] = json!("PRODUCT");
    } else {
        row["qstJson"] = json!("");
    }
    Ok(row)
}

/// Build the legacy flipbook reader's `TaskStuded` row without sending it.
///
/// The extracted `packType < 7` reader first persists per-question
/// `elementAttr` rows through `submitEleAttr`. This builder emits the
/// flattened `qstJson` upload rows directly; it does not persist the reader's
/// local `TaskStuded` record.
pub fn build_legacy_reader_row(
    context: &ReadAloudContext,
    questions: &[PaperQuestion],
    recordings: &[ReadAloudRecording],
    scores: &[f64],
    study_date_ms: i64,
    seconds: i64,
) -> Result<Value> {
    if questions.len() != recordings.len() || questions.len() != scores.len() {
        return Err(ProtocolError::Invalid(
            "legacy reader row inputs have mismatched lengths".to_owned(),
        ));
    }

    let mut qst_json = Vec::with_capacity(
        questions
            .iter()
            .map(|question| question.sentences.len() + 1)
            .sum(),
    );
    let mut total: f64 = 0.0;
    let max_score: f64 = questions.iter().map(|question| question.score).sum();
    for ((question, recording), score) in questions.iter().zip(recordings).zip(scores) {
        let scored = score_recording(question, recording, *score, study_date_ms)?;
        total += score;
        for element in qst_elements(
            &question.question_id,
            question.sentences.len(),
            &scored.element_attr,
        ) {
            let element_attr = serde_json::to_string(&element.element_attr)
                .map_err(|error| ProtocolError::Decode(format!("legacy qstJson: {error}")))?;
            qst_json.push(json!({
                "pageId": context.content_id,
                "elementId": element.element_id,
                "elementType": element.element_type,
                "addTime": study_date_ms,
                "order": element.order,
                "elementAttr": element_attr,
            }));
        }
    }

    Ok(json!({
        "id": uuid::Uuid::new_v4().simple().to_string(),
        "uid": context.uid,
        "bookId": context.book_id,
        "taskId": context.content_id,
        "chapterId": context.chapter_id,
        "taskNo": 1,
        "score": total,
        "percent": score_percent(total, max_score),
        "studyDate": study_date_ms,
        "seconds": seconds,
        "result": "",
        "srcType": 1,
        "batchTag": uuid::Uuid::new_v4().simple().to_string(),
        "uploaded": 0,
        "msg": "",
        "failed": 0,
        "studyTaskId": context.job_id.clone().unwrap_or_default(),
        "courseId": context.course_id,
        "extendParams": {},
        "qstJson": qst_json,
        "killStudyTask": 0,
        "submitType": 0,
    }))
}

/// Send one constructed row.
///
/// The caller must have shown the row to the user first: this is a write and
/// upstream rejects a second submission of the same job with `-13`.
pub async fn submit_readaloud(client: &Client, row: &Value, timestamp_ms: u128) -> Result<Value> {
    let endpoint = get("task_score_submit")
        .ok_or_else(|| ProtocolError::Decode("missing endpoint: task_score_submit".into()))?;
    let tasks_json = serde_json::to_string(&json!([row]))
        .map_err(|error| ProtocolError::Decode(format!("tasksJson: {error}")))?;
    let ut = submit_task_ut(&tasks_json, timestamp_ms);
    // The form field must match the row: a course-book page submits type 0,
    // a scheduled job type 8.
    let submit_type = row
        .get("submitType")
        .and_then(Value::as_i64)
        .unwrap_or(READALOUD_SUBMIT_TYPE);
    let data = client
        .call(
            endpoint,
            &[
                ("tasksJson", json!(tasks_json)),
                ("submitType", json!(submit_type)),
                ("ut", json!(ut)),
            ],
        )
        .await?;
    Ok(data.unwrap_or(Value::Null))
}

/// Print an integer score as the client does (`8.0`, not `8`).
fn score_text(score: f64) -> String {
    if score.fract().abs() < f64::EPSILON {
        format!("{score:.1}")
    } else {
        score.to_string()
    }
}

/// Split a CDATA terminator so user text cannot close the section early.
fn cdata(text: &str) -> String {
    text.replace("]]>", "]]]]><![CDATA[>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::readaloud::paper::PaperQuestion;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    fn paper() -> ReadAloudPaper {
        ReadAloudPaper {
            questions: vec![
                PaperQuestion {
                    question_id: "Q1".to_owned(),
                    question_type: 9,
                    number: 1,
                    score: 10.0,
                    duration: 82,
                    sentences: vec!["One.".to_owned(), "Two.".to_owned()],
                    audio_url: None,
                },
                PaperQuestion {
                    question_id: "Q2".to_owned(),
                    question_type: 9,
                    number: 2,
                    score: 10.0,
                    duration: 86,
                    sentences: vec!["Three.".to_owned()],
                    audio_url: None,
                },
            ],
            non_read_aloud: 0,
            sentence_count: 3,
            total_score: 20.0,
        }
    }

    fn recordings() -> Vec<ReadAloudRecording> {
        vec![
            ReadAloudRecording {
                question_id: "Q1".to_owned(),
                record_url: "https://oss/q1.mp3".to_owned(),
                duration_ms: None,
            },
            ReadAloudRecording {
                question_id: "Q2".to_owned(),
                record_url: "https://oss/q2.mp3".to_owned(),
                duration_ms: None,
            },
        ]
    }

    fn context() -> ReadAloudContext {
        ReadAloudContext {
            uid: 39370562,
            client_id: "CID".to_owned(),
            course_id: "9222783".to_owned(),
            book_id: "BOOK".to_owned(),
            content_id: "CONTENT".to_owned(),
            chapter_id: "CHAPTER".to_owned(),
            job_id: Some("JOB".to_owned()),
            paper_id: "PAPER".to_owned(),
        }
    }

    #[test]
    fn the_row_carries_one_element_per_question_and_sentence() {
        let row = build_row(
            &paper(),
            &context(),
            &recordings(),
            &[8.0, 8.0],
            1_793_714_405_924,
            303,
        )
        .unwrap();

        assert_eq!(row["studyTaskId"], "JOB");
        assert_eq!(row["taskId"], "CONTENT");
        assert_eq!(row["chapterId"], "CHAPTER");
        assert_eq!(row["score"], 16.0);
        assert_eq!(row["percent"], 80.0);
        // Two parent rows plus three sentence rows.
        assert_eq!(row["qstJson"].as_array().unwrap().len(), 5);

        let result: Value = serde_json::from_str(row["result"].as_str().unwrap()).unwrap();
        let answer = result["testInfo"]["answerdata"].as_str().unwrap();
        assert!(answer.contains("https://oss/q1.mp3"));
        assert!(answer.contains("<answer_score>8.0</answer_score>"));
        assert!(answer.contains("<answer_recordsec><![CDATA[82]]></answer_recordsec>"));
        let marks = result["testInfo"]["markdatao"].as_str().unwrap();
        assert!(
            marks.contains("<element id=\"Q1\"><mark_data><score>8.0</score><result>3</result>")
        );
    }

    #[test]
    fn a_book_page_row_has_no_job_fields() {
        let mut context = context();
        context.job_id = None;
        let row = build_row(
            &paper(),
            &context,
            &recordings(),
            &[8.0, 8.0],
            1_793_714_405_924,
            303,
        )
        .unwrap();

        assert_eq!(row["studyTaskId"], "");
        assert_eq!(row["submitType"], BOOK_TASK_SUBMIT_TYPE);
        assert_eq!(row["qstJson"], "");
        assert!(row.get("paperId").is_none());
        assert!(row.get("clientId").is_none());
        assert!(row.get("deviceType").is_none());
        assert!(row.get("wrongnoteFlag").is_none());
    }

    #[test]
    fn uploaded_duration_overrides_zero_paper_duration() {
        let mut paper = paper();
        paper.questions[0].duration = 0;
        let mut recording = recordings();
        recording[0].duration_ms = Some(152_111);

        let row = build_row(
            &paper,
            &context(),
            &recording,
            &[8.0, 8.0],
            1_793_714_405_924,
            303,
        )
        .unwrap();

        let result: Value = serde_json::from_str(row["result"].as_str().unwrap()).unwrap();
        let answer = result["testInfo"]["answerdata"].as_str().unwrap();
        assert!(answer.contains("<answer_recordsec><![CDATA[153]]></answer_recordsec>"));
        assert_eq!(row["qstJson"][0]["elementAttr"]["duration"], 153);
    }

    #[test]
    fn legacy_reader_row_uses_src_type_one_and_qst_json() {
        let row = build_legacy_reader_row(
            &context(),
            &paper().questions,
            &recordings(),
            &[8.0, 8.0],
            1_793_714_405_924,
            303,
        )
        .unwrap();

        assert_eq!(row["srcType"], 1);
        assert_eq!(row["result"], "");
        assert_eq!(row["studyTaskId"], "JOB");
        assert_eq!(row["submitType"], 0);
        assert!(row["extendParams"].is_object());
        assert_eq!(row["qstJson"].as_array().unwrap().len(), 5);
        assert_eq!(row["qstJson"][0]["elementType"], 1);
        assert_eq!(row["qstJson"][1]["elementType"], 2);
        assert!(row["qstJson"][0]["elementAttr"].is_string());
    }
    #[test]
    fn a_missing_recording_or_out_of_range_score_is_rejected() {
        assert!(build_row(&paper(), &context(), &recordings()[..1], &[8.0, 8.0], 1, 1,).is_err());
        assert!(build_row(&paper(), &context(), &recordings(), &[11.0, 8.0], 1, 1,).is_err());
    }

    #[test]
    fn qst_json_keeps_the_question_own_recording_type() {
        let mut typed = paper();
        typed.questions[1].question_type = 13;
        let row = build_row(
            &typed,
            &context(),
            &recordings(),
            &[8.0, 8.0],
            1_793_714_405_924,
            303,
        )
        .unwrap();

        let rows = row["qstJson"].as_array().unwrap();
        let parents: Vec<_> = rows
            .iter()
            .filter(|entry| entry["elementType"] == 1)
            .map(|entry| entry["elementAttr"]["question_type"].clone())
            .collect();
        assert_eq!(parents, vec![json!(9), json!(13)]);
    }

    #[test]
    fn a_cdata_terminator_in_the_text_is_split() {
        assert_eq!(cdata("a ]]> b"), "a ]]]]><![CDATA[> b");
    }

    #[tokio::test]
    async fn submit_posts_the_confirmed_form_fields() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/task/score/submit/v2")
                .body_includes("submitType=8")
                .body_includes("tasksJson")
                .body_includes("ut=");
            then.status(200)
                .json_body(ok(json!({"code": 0, "msg": "success"})));
        });

        let row = build_row(&paper(), &context(), &recordings(), &[8.0, 8.0], 1, 1).unwrap();
        submit_readaloud(&client(&server), &row, 1_700_000_000_000)
            .await
            .unwrap();
        mock.assert_calls(1);
    }
}
