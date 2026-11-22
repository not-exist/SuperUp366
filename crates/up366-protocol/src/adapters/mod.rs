//! Read-only adapters over the endpoint table.
//!
//! Each adapter owns one upstream call: it sends the exact parameters the PC
//! client sends, then decodes the payload into a DTO from [`crate::dto`]. All
//! functions take `&Client`, so a single authenticated client can be shared by
//! the sync layer.

pub mod answers;
pub mod books;
pub mod catalog;
pub mod correction;
mod paper;
pub mod profile;
pub mod readaloud;
pub mod wordpk;
pub mod words;
pub mod writing;

pub use answers::{
    answer_detail, answer_detail_v1, answer_detail_v2, paper_detail, textbook_paperinfo,
};
pub use books::{
    book_info, book_plan, book_task_nodes, book_task_questions, book_task_words, book_tree_pages,
    books, course_books,
};
pub use catalog::{course_finished_jobs, courses, finished_tasks, history_jobs, job_info, jobs};
pub use correction::{
    build_tasks_json, clear_wrong_notes, correction_draw, correction_overview, correction_status,
    mark_xml, qst_json, submit_correction, CorrectionAnswer, CorrectionContext, CorrectionGroup,
    CorrectionOverview, CorrectionPaper, CorrectionQuestion, CorrectionStatus,
};
pub use profile::{current_user, server_time, CurrentUser};
pub use readaloud::{
    build_legacy_reader_row, build_row, parse_paper, submit_readaloud, upload_object,
    upload_ticket, PaperQuestion, ReadAloudContext, ReadAloudPaper, ReadAloudRecording,
    UploadTicket, READALOUD_SUBMIT_TYPE,
};
pub use wordpk::{submit_word_pk, WordPkSubmission};
pub use words::bucket_words;
pub use writing::{
    build_answer_data, parse_applied_writing, submit_applied_writing, AppliedWritingPaper,
    AppliedWritingSubmission, WritingAnswer, WritingQuestion, APPLIED_WRITING_QUESTION_TYPE,
    MAX_WRITING_CHARS,
};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::dto::{self, RawPayload};
use crate::endpoint::{get, Endpoint};
use crate::error::{ProtocolError, Result};

/// Look up a frozen endpoint, failing loudly if the table is ever edited wrong.
pub(crate) fn endpoint(name: &str) -> Result<&'static Endpoint> {
    get(name).ok_or_else(|| ProtocolError::Decode(format!("missing endpoint: {name}")))
}

/// Decode every item of a list response.
pub(crate) fn decode_items<T>(items: Vec<Value>, endpoint: &str) -> Result<Vec<T>>
where
    T: DeserializeOwned + RawPayload,
{
    items
        .into_iter()
        .map(|item| dto::decode(item, endpoint))
        .collect()
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::client::Client;
    use crate::session::Session;
    use serde_json::{json, Value};
    use std::time::Duration;

    pub fn client(server: &httpmock::MockServer) -> Client {
        Client::builder()
            .client_id("CID-TEST")
            .session(Session::new("alice", "TGT-1", "CID-TEST"))
            .base_url_override(server.base_url())
            .retry_base_delay(Duration::ZERO)
            .clock(|| 1_700_000_000_000)
            .build()
            .unwrap()
    }

    pub fn ok(data: Value) -> Value {
        json!({ "result": { "code": 0, "msg": "ok" }, "data": data })
    }
}
