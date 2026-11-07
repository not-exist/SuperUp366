//! Read-aloud commands.
//!
//! This module wires the read-aloud path to the guarded protocol layer
//! (`up366_protocol::adapters::readaloud`). Two facts decide what it offers:
//!
//! 1. **Upload and submit are separate.** Audio is uploaded to OSS first and
//!    referenced by URL; the score payload only carries URLs. Uploading is the
//!    reversible half, submitting is not.
//! 2. **The submit payload is not confirmed yet** (`docs/protocol/to-probe.md`
//!    §4). So `readaloud_submit` produces a preview and, unless the caller
//!    explicitly opts out of the dry run, does not send.
//!
//! The slot list itself needs the paper, which is parsed in the sync layer;
//! until that lands this module reports the task as having no slots rather
//! than inventing them.

use serde::{Deserialize, Serialize};
use up366_protocol::adapters::{preview_submission, submit_readaloud, ReadAloudTaskView};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// A read-aloud task as the upload page renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudView {
    pub job_id: String,
    pub title: String,
    pub submitted: bool,
    /// Slots are only known once the paper is parsed.
    pub slots: Vec<SlotView>,
    /// Always false until the submission shape is confirmed.
    pub submission_enabled: bool,
}

/// One recording slot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlotView {
    pub question_id: String,
    pub number: usize,
    pub sentence: String,
    /// Set once a recording has been uploaded for this slot.
    pub record_url: Option<String>,
}

/// A validated local recording, ready to be uploaded.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UploadView {
    pub ok: bool,
    pub question_id: String,
    pub file_name: String,
    pub size: u64,
    /// `None` until the OSS transfer is wired; the UI must not show "uploaded".
    pub record_url: Option<String>,
}

/// What the confirmation dialog shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudPreviewView {
    pub job_id: String,
    pub title: String,
    /// The exact payload a submission would carry.
    pub payload: serde_json::Value,
    /// True when nothing was sent.
    pub dry_run: bool,
}

/// The recording the user picked for one slot.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RecordingUpload {
    pub question_id: String,
    /// Local file path of the mp3.
    pub path: String,
    /// File name, used to build the OSS object name.
    pub file_name: String,
}

/// Read the task's read-aloud slots.
///
/// Slots come from the paper XML, which the sync layer parses; until that
/// exists this reports an honest empty list rather than a guessed one, and the
/// caller is told why.
#[tauri::command]
pub async fn readaloud_get(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudView> {
    // Validate the account even though nothing is fetched: a request scoped to
    // a non-active account must fail, not silently read the active one.
    state.require_client(Some(&account_id))?;
    Ok(ReadAloudView {
        job_id: job_id.clone(),
        title: job_id,
        submitted: false,
        slots: Vec::new(),
        submission_enabled: READALOUD_SUBMISSION_ENABLED,
    })
}

/// Validate one picked mp3 and report what would be uploaded.
///
/// Returns the validated local file: the caller shows the slot as "ready", not
/// "uploaded", because the OSS transfer is not wired yet. Nothing is sent.
#[tauri::command]
pub async fn readaloud_upload(
    account_id: String,
    _job_id: String,
    recording: RecordingUpload,
    state: tauri::State<'_, AppState>,
) -> CommandResult<UploadView> {
    state.require_client(Some(&account_id))?;

    let path = std::path::Path::new(&recording.path);
    let info = up366_media::inspect_mp3(path)
        .map_err(|error| AppError::new("invalid_input", format!("音频文件不可用: {error}")))?;

    Ok(UploadView {
        ok: true,
        question_id: recording.question_id,
        file_name: recording.file_name,
        size: info.size,
        record_url: None,
    })
}

/// Produce the request a submission would send, without sending it.
///
/// `dry_run` defaults to true. Passing `false` reaches the guarded protocol
/// layer, which still refuses while the payload shape is unconfirmed.
#[tauri::command]
pub async fn readaloud_submit(
    account_id: String,
    job_id: String,
    dry_run: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudPreviewView> {
    let (_, client) = state.require_client(Some(&account_id))?;
    let dry_run = dry_run.unwrap_or(true);

    let task: ReadAloudTaskView = serde_json::from_value(serde_json::json!({
        "jobId": job_id,
        "title": job_id,
        "submitted": false,
        "slots": [],
    }))
    .map_err(|error| AppError::new("protocol_drift", error.to_string()))?;

    let payload = preview_submission(&task).map_err(AppError::from)?;
    if dry_run {
        let _ = client;
        return Ok(ReadAloudPreviewView {
            job_id: task.job_id.clone(),
            title: task.title.clone(),
            payload,
            dry_run: true,
        });
    }

    submit_readaloud(&client, &task, false)
        .await
        .map_err(AppError::from)?;
    Ok(ReadAloudPreviewView {
        job_id: task.job_id,
        title: task.title,
        payload,
        dry_run: false,
    })
}

/// Whether the score payload has been confirmed against a real task.
///
/// Flips to true only when `docs/protocol/to-probe.md` §4 items 1–3 are closed.
const READALOUD_SUBMISSION_ENABLED: bool = false;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_start_empty_and_submission_stays_disabled() {
        // A task with no paper parsed must not invent slots, and must not offer
        // a submit button the backend would refuse.
        let view = ReadAloudView {
            job_id: "JOB-1".to_owned(),
            title: "Unit 4".to_owned(),
            submitted: false,
            slots: Vec::new(),
            submission_enabled: READALOUD_SUBMISSION_ENABLED,
        };
        assert!(view.slots.is_empty());
        assert!(!view.submission_enabled);
    }

    #[test]
    fn a_preview_carries_the_job_and_marks_itself_as_a_dry_run() {
        let payload = serde_json::json!({"jobId": "JOB-1"});
        let view = ReadAloudPreviewView {
            job_id: "JOB-1".to_owned(),
            title: "Unit 4".to_owned(),
            payload,
            dry_run: true,
        };
        assert!(view.dry_run);
        assert_eq!(view.payload["jobId"], "JOB-1");
    }
}
