//! Word-PK write adapter.
//!
//! The submission shape comes from the reference client: the task-mode submit
//! sends a compact `submitJson` with the round's score and one
//! `answerStatus`-annotated word per attempted word.
//!
//! Method caveat: `recon/feature-word-pk.md` records **GET**, while the retired
//! client sent POST and only fell back to GET on 404/405. The endpoint table
//! therefore says POST and the mismatch is tracked in
//! `docs/protocol/to-probe.md`. Nothing here "tries" a method at runtime: a
//! burned attempt cannot be un-burned, so the shape is fixed at build time and
//! a probe decides it before the feature is enabled.

use serde_json::{json, Value};

use crate::client::Client;

use crate::dto::Word;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};

/// One round of word PK to submit.
#[derive(Debug, Clone)]
pub struct WordPkSubmission {
    /// `courseId` of the task.
    pub course_id: String,
    /// `studyTaskId`, which equals the job id.
    pub study_task_id: String,
    /// Words answered correctly, in the order they were shown.
    pub correct: Vec<Word>,
    /// Words answered incorrectly.
    pub wrong: Vec<Word>,
    /// Requested round score; the server derives it from the word list, so the
    /// value is only sent, never trusted.
    pub score: i64,
    /// Round duration as sent by the reference client; empty when unknown.
    pub duration: String,
}

impl WordPkSubmission {
    /// `total` = every attempted word.
    pub fn total(&self) -> usize {
        self.correct.len() + self.wrong.len()
    }

    /// `rightNum`; must equal `score` for a consistent round.
    pub fn right(&self) -> usize {
        self.correct.len()
    }

    /// Word records in the wire order: correct first, then wrong.
    fn word_infos(&self) -> Vec<Value> {
        let mut infos = Vec::with_capacity(self.total());
        for (status, words) in [(1, &self.correct), (0, &self.wrong)] {
            for (index, word) in words.iter().enumerate() {
                let entry_id = word
                    .entry_id
                    .clone()
                    .or_else(|| word.entry.clone())
                    .unwrap_or_default();
                infos.push(json!({
                    "entryId": entry_id,
                    "answerStatus": status,
                    "localId": infos.len() + index,
                }));
            }
        }
        infos
    }

    /// The exact `submitJson` payload text.
    fn submit_json(&self) -> String {
        let payload = json!({
            "totalNum": self.total(),
            "rightNum": self.right(),
            "score": self.score,
            "wordInfos": self.word_infos(),
            "duration": self.duration,
        });
        // Compact separators match the reference client byte for byte.
        serde_json::to_string(&payload).expect("payload is serializable")
    }

    /// The full form body.
    fn form(&self) -> Vec<(&'static str, Value)> {
        vec![
            ("courseId", json!(&self.course_id)),
            ("studyTaskId", json!(&self.study_task_id)),
            ("roleName", json!("Red")),
            ("roleType", json!("0")),
            ("submitJson", json!(self.submit_json())),
            ("wrongLength", json!(self.wrong.len())),
            ("score", json!(self.score)),
        ]
    }
}

/// Submit one round.
///
/// This is a real write. Callers must have shown a preview and obtained a
/// confirmation token; the adapter performs no confirmation of its own.
pub async fn submit_word_pk(client: &Client, submission: &WordPkSubmission) -> Result<Value> {
    let endpoint = get("word_pk_submit")
        .ok_or_else(|| ProtocolError::Decode("missing endpoint: word_pk_submit".into()))?;

    if submission.total() == 0 {
        return Err(ProtocolError::Decode(
            "word pk round has no words".to_owned(),
        ));
    }
    if submission.correct.len() as i64 != submission.score {
        // The server derives the score from the correct list, so a mismatch
        // means the caller computed the preview from different data.
        return Err(ProtocolError::Decode(
            "score does not match the number of correct words".to_owned(),
        ));
    }

    let data = client.call(endpoint, &submission.form()).await?;
    Ok(data.unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    fn word(entry_id: &str, entry: &str) -> Word {
        serde_json::from_value(serde_json::json!({
            "entryId": entry_id,
            "entry": entry
        }))
        .unwrap()
    }

    fn round() -> WordPkSubmission {
        WordPkSubmission {
            course_id: "9222783".to_owned(),
            study_task_id: "JOB-1".to_owned(),
            correct: vec![word("e1", "apple")],
            wrong: vec![word("e2", "banana")],
            score: 1,
            duration: String::new(),
        }
    }

    #[test]
    fn submit_json_counts_and_orders_words() {
        let json: Value = serde_json::from_str(&round().submit_json()).unwrap();
        assert_eq!(json["totalNum"], 2);
        assert_eq!(json["rightNum"], 1);
        assert_eq!(json["score"], 1);
        let infos = json["wordInfos"].as_array().unwrap();
        assert_eq!(infos.len(), 2);
        // Correct first.
        assert_eq!(infos[0]["entryId"], "e1");
        assert_eq!(infos[0]["answerStatus"], 1);
        assert_eq!(infos[1]["entryId"], "e2");
        assert_eq!(infos[1]["answerStatus"], 0);
    }

    #[test]
    fn submit_json_is_compact_like_the_reference_client() {
        let json = round().submit_json();
        assert!(!json.contains(", ") && !json.contains(": "));
    }

    #[test]
    fn a_round_without_words_is_rejected_before_any_request() {
        let mut round = round();
        round.correct.clear();
        round.wrong.clear();
        round.score = 0;
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/wordsbtl/student/submit");
            then.status(200).json_body(ok(json!({})));
        });

        let error = tokio_test_block(submit_word_pk(&client(&server), &round));
        assert!(error.is_err());
    }

    #[tokio::test]
    async fn a_score_that_disagrees_with_the_word_list_is_rejected() {
        let mut round = round();
        round.score = 5;
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/wordsbtl/student/submit");
            then.status(200).json_body(ok(json!({})));
        });

        assert!(submit_word_pk(&client(&server), &round).await.is_err());
        // Nothing was sent: a mismatched score would corrupt the round.
        mock.assert_hits(0);
    }

    #[tokio::test]
    async fn the_form_body_carries_every_field_the_client_sends() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/wordsbtl/student/submit")
                .body_contains("courseId=9222783")
                .body_contains("studyTaskId=JOB-1")
                .body_contains("roleName=Red")
                .body_contains("roleType=0")
                .body_contains("wrongLength=1")
                .body_contains("score=1")
                .body_contains("submitJson=");
            then.status(200).json_body(ok(json!({"rc": 0})));
        });

        submit_word_pk(&client(&server), &round()).await.unwrap();
        mock.assert_hits(1);
    }

    /// Sync helper for the non-async test above.
    fn tokio_test_block<F>(future: F) -> Result<Value>
    where
        F: std::future::Future<Output = Result<Value>>,
    {
        tokio::runtime::Runtime::new().unwrap().block_on(future)
    }
}
