//! Wrong-answer book adapters: reads, the kill paper, and the cleanup write.
//!
//! Read/write boundary is the important part here:
//!
//! - `questions/kill` is a **read**: it returns the paper the student must
//!   answer to clear wrong questions.
//! - `question/submit` is the only destructive call.
//!
//! Cleanup is inherently batch-shaped (one submission per question), and the
//! granularity the server accepts is not confirmed (`docs/protocol/to-probe.md`
//! §6). So the write path is deliberately built so that:
//!
//! - a single question is the unit, and the caller decides how many to send;
//! - nothing is retried automatically — `-13`/`-21` identify a burned or dead
//!   attempt, and a timeout is a `Transport` error the caller must resolve;
//! - the per-question result is returned so a partial batch can be resumed.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client::Client;
use crate::codec;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};

/// One subject row from `subject/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongSubject {
    pub subject_id: String,
    pub subject_name: String,
    pub stage_id: Option<String>,
    pub book_id: Option<String>,
    pub wrong_count: i64,
    pub killed_count: i64,
}

/// One knowledge-point row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongKnowledge {
    pub knowledge_id: String,
    pub knowledge_name: String,
    pub qtype_id: String,
    pub qtype_name: String,
    pub wrong_count: i64,
}

/// One un-cleared question from `student/question/show`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongQuestion {
    pub question_id: String,
    pub title: String,
    pub question_type: String,
    pub stem: String,
    pub correct_answer: String,
    pub student_answer: String,
}

/// The paper a student must answer to clear questions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillPaper {
    pub path_prefix: String,
    pub paper_xml: String,
    pub answer_xml: String,
}

/// Paging parameters shared by the wrong-note list endpoints.
///
/// Both are optional because the archive is inconsistent about whether the
/// endpoints need a date range; each call sends only what it has.
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
    let items = codec::into_array(data, endpoint.name)?;
    items
        .into_iter()
        .map(|item| decode(item, endpoint.name))
        .collect()
}

/// Knowledge points for one subject/book.
pub async fn wrong_knowledge(client: &Client, query: &WrongQuery) -> Result<Vec<WrongKnowledge>> {
    let endpoint = endpoint("wrong_knowledge")?;
    let data = client.call(endpoint, &query.form()).await?;
    let items = codec::into_array(data, endpoint.name)?;
    items
        .into_iter()
        .map(|item| decode(item, endpoint.name))
        .collect()
}

/// The questions still to clear for one knowledge point.
pub async fn wrong_pending(client: &Client, query: &WrongQuery) -> Result<Vec<WrongQuestion>> {
    let endpoint = endpoint("wrong_question_show")?;
    let data = client.call(endpoint, &query.form()).await?;
    // `data.list` holds the question summaries; an empty object means "no
    // questions", which decodes to an empty page rather than an error.
    let mut rows = codec::into_object(data, endpoint.name)?;
    let items = codec::take_list(&mut rows, "list", endpoint.name)?;
    items
        .into_iter()
        .map(|item| decode(item, endpoint.name))
        .collect()
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
    /// `<answerdata>` fragment for this question.
    pub answer_xml: String,
    /// `<mark_datao>` fragment for this question.
    pub mark_xml: String,
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
    pub seconds: i64,
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

    #[test]
    fn cleanup_form_carries_the_six_fields_and_derives_seconds() {
        let cleanup = WrongCleanup {
            uid: "42".to_owned(),
            answer_xml: "<answerdata/>".to_owned(),
            mark_xml: "<elements/>".to_owned(),
            start_time: 1_000,
            end_time: 4_500,
            seconds: 99, // must be ignored in favour of the derived value
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
            seconds: 1,
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
            seconds: 0,
            right_qst_ids: String::new(),
        };
        // No server is needed: the guard fires before the transport.
        assert!(matches!(
            tokio_test(submit_wrong_question(&Client::new().unwrap(), &cleanup)),
            Err(ProtocolError::Invalid(_))
        ));
    }

    fn tokio_test<F>(future: F) -> Result<Value>
    where
        F: std::future::Future<Output = Result<Value>>,
    {
        tokio::runtime::Runtime::new().unwrap().block_on(future)
    }
}
