//! Read-aloud: paper parsing, OSS transfer and the score submission.
//!
//! The confirmed scheduled-job flow (two live submissions,
//! `docs/protocol/confirmed.md`):
//!
//! 1. `paper/detail` gives the paper XML; every `question_type = 9` element is
//!    one recording slot and its `<attachment>` lists the sentences.
//! 2. The mp3 is uploaded to OSS; the URL is the only audio reference the
//!    server sees.
//! 3. One row is posted to `/client/task/score/submit/v2` with
//!    `studyTaskId = jobId`, `submitType = 8` and the `tasksJson`/`ut` pair.
//!
//! Course-book pages use a separate legacy flipbook row builder when their
//! `packType < 7` selects that reader. The app actively uses this path, but
//! its wire shape is based on static extraction and is not yet confirmed
//! end to end.
//!
//! Upload and submit are separate on purpose: the upload is reversible, the
//! submit is not (a duplicate returns `-13` and burns the job).

mod oss;
mod paper;
mod payload;

pub use oss::{upload_object, upload_ticket, UploadTicket};
pub use paper::{parse_paper, PaperQuestion, ReadAloudPaper};
pub use payload::{
    build_legacy_reader_row, build_row, submit_readaloud, ReadAloudContext, ReadAloudRecording,
    READALOUD_SUBMIT_TYPE,
};
