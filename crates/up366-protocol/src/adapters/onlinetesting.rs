//! Whole-document submission: `xot-api /client/onlinetesting/submit`.
//!
//! The official homework player posts the answer XML of every question in one
//! form body. There is no `ut`, no gzip and no `markData`; the server grades
//! the document asynchronously. Applied writing and dictation both use this
//! route, so the request lives here once.

use serde_json::{json, Value};

use crate::client::Client;
use crate::error::Result;

/// One whole-document submission.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSubmission {
    /// The page's `testId`, which is the job's `contentId`.
    pub test_id: String,
    pub paper_id: String,
    pub uid: i64,
    pub answer_data: String,
    /// Seconds spent on the paper; the server records it as study time.
    pub duration: i64,
    /// Epoch milliseconds.
    pub begin_time: i64,
    /// Epoch milliseconds.
    pub end_time: i64,
}

/// Post one answer document.
///
/// The caller must have shown the exact XML to the user first: this is a write,
/// and a second post of an already-submitted job is silently ignored by the
/// server unless `homework_test_redo` reset it first.
pub async fn submit_document(client: &Client, submission: &DocumentSubmission) -> Result<Value> {
    let endpoint = super::endpoint("onlinetesting_submit")?;
    let data = client
        .call(
            endpoint,
            &[
                ("testId", json!(submission.test_id)),
                // The Electron queue stores `taskId` and copies it into
                // `testId` on send; both are the job's content id.
                ("taskId", json!(submission.test_id)),
                ("paperId", json!(submission.paper_id)),
                ("uid", json!(submission.uid)),
                ("answerData", json!(submission.answer_data)),
                ("duration", json!(submission.duration.max(1))),
                ("beginTime", json!(submission.begin_time)),
                ("endTime", json!(submission.end_time)),
                ("studyDate", json!("")),
                ("playData", json!("")),
                ("timeData", json!("")),
                ("uploaded", json!(0)),
                ("failed", json!(0)),
                ("msg", json!("")),
            ],
        )
        .await?;
    Ok(data.unwrap_or(Value::Null))
}
