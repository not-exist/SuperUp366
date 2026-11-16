//! Read-aloud score payload.
//!
//! Shape confirmed against two live submissions (see
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

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::paper::ReadAloudPaper;
use crate::client::Client;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};
use crate::signature::submit_task_ut;

/// `submitType` the textbook read-aloud branch uses.
pub const READALOUD_SUBMIT_TYPE: i64 = 8;
const DEVICE_TYPE: &str = "Win10";

/// One uploaded recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadAloudRecording {
    pub question_id: String,
    /// OSS URL returned by [`super::upload_object`]'s caller.
    pub record_url: String,
}

/// Ids every read-aloud submission needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAloudContext {
    pub uid: i64,
    pub client_id: String,
    pub course_id: String,
    pub book_id: String,
    /// `taskId` = `chapterId` = `contentId`.
    pub content_id: String,
    /// `studyTaskId` must be this, not a generated id.
    pub job_id: String,
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
    let mut qst_json = Vec::new();
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
        let result = if score >= question.score {
            1
        } else if score == 0.0 {
            2
        } else {
            3
        };
        total += score;

        let text = question.sentences.join(" ");
        let question_id = question.question_id.as_str();
        let record_url = recording.record_url.as_str();
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
            duration = question.duration,
        ));
        mark_elements.push(format!(
            "<element id=\"{id}\"><mark_data><score>{score}</score>\
             <result>{result}</result></mark_data></element>",
            id = question_id,
            score = score_text(score),
        ));

        let attr = json!({
            "question_id": question_id,
            "question_type": question.question_type,
            "order": 1,
            "user_score": score,
            "user_answer": "",
            "answer": "",
            "score": question.score,
            "result": result,
            "timestamp": study_date_ms,
            "record_url": record_url,
            "duration": question.duration,
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
        qst_json.push(json!({
            "elementId": question_id,
            "elementType": 1,
            "elementAttr": attr,
        }));
        for number in 1..=question.sentences.len() {
            let element_id = format!("{question_id}-{number}");
            let mut sentence_attr = attr.clone();
            sentence_attr["question_id"] = json!(&element_id);
            sentence_attr["order"] = json!(number);
            qst_json.push(json!({
                "elementId": &element_id,
                "elementType": 2,
                "elementAttr": sentence_attr,
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
    let percent = if paper.total_score > 0.0 {
        (total / paper.total_score * 100.0 * 100.0).round() / 100.0
    } else {
        0.0
    };

    Ok(json!({
        "id": uuid::Uuid::new_v4().simple().to_string(),
        "taskId": context.content_id,
        "uid": context.uid,
        "bookId": context.book_id,
        "chapterId": context.content_id,
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
        "studyTaskId": context.job_id,
        "courseId": context.course_id,
        "paperId": context.paper_id,
        "extendParams": { "batchId": uuid::Uuid::new_v4().simple().to_string() },
        "qstJson": qst_json,
        "failed": 0,
        "killStudyTask": 0,
        "submitType": READALOUD_SUBMIT_TYPE,
        "deviceType": DEVICE_TYPE,
        "clientId": context.client_id,
        "wrongnoteFlag": "PRODUCT",
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
    let data = client
        .call(
            endpoint,
            &[
                ("tasksJson", json!(tasks_json)),
                ("submitType", json!(READALOUD_SUBMIT_TYPE)),
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
            },
            ReadAloudRecording {
                question_id: "Q2".to_owned(),
                record_url: "https://oss/q2.mp3".to_owned(),
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
            job_id: "JOB".to_owned(),
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
        assert_eq!(row["chapterId"], "CONTENT");
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
