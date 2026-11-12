//! Read-aloud commands: detect slots, upload mp3s, preview, submit once.
//!
//! The flow follows the confirmed protocol (see
//! `docs/protocol/confirmed.md`):
//!
//! 1. `readaloud_get` resolves the job's paper and parses every
//!    `question_type = 9` element. The slot count is therefore **detected from
//!    the paper**, never guessed.
//! 2. `readaloud_upload` validates and uploads one mp3, storing the URL.
//! 3. `readaloud_preview` builds the exact score row and issues a single-use
//!    token.
//! 4. `readaloud_submit` consumes the token, posts once, then reads the
//!    finished-job list back.
//!
//! Expired jobs are not blocked: the server accepted late submissions during
//! recon, so the UI only warns.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use up366_core::{AccountId, JobId};
use up366_protocol::adapters::{self, ReadAloudContext, ReadAloudPaper, ReadAloudRecording};
use up366_protocol::dto::Job;
use up366_store::JobRecord;

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// One recording slot as the upload page renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudSlotView {
    pub question_id: String,
    pub number: usize,
    /// Sentences joined with spaces; kept as one string for display.
    pub sentence: String,
    pub sentences: Vec<String>,
    pub score: f64,
    pub duration: u32,
    pub audio_url: Option<String>,
    /// Set once a recording has been uploaded for this slot.
    pub record_url: Option<String>,
}

/// A read-aloud task, with the slot count detected from the paper.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudTaskView {
    pub job_id: String,
    pub title: String,
    pub submitted: bool,
    /// Expired jobs can still be submitted (late submission worked upstream).
    pub expired: bool,
    pub question_count: usize,
    pub sentence_count: usize,
    pub total_score: f64,
    pub slots: Vec<ReadAloudSlotView>,
}

/// A validated and uploaded local recording.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UploadView {
    pub ok: bool,
    pub question_id: String,
    pub file_name: String,
    pub size: u64,
    pub record_url: String,
}

/// One question in the confirmation dialog.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreviewSlotView {
    pub question_id: String,
    pub number: usize,
    pub sentence: String,
    pub score: f64,
    pub record_url: String,
}

/// What the confirmation dialog shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudPreviewView {
    pub job_id: String,
    pub title: String,
    pub total_score: f64,
    pub reported_score: f64,
    /// The exact row a submission would carry.
    pub row: Value,
    pub slots: Vec<PreviewSlotView>,
    /// Single-use token for `readaloud_submit`.
    pub confirm_token: String,
}

/// The result of one submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadAloudSubmitView {
    pub ok: bool,
    /// Whether the finished-job list already shows the job.
    pub verified: bool,
    pub score: Option<f64>,
    pub batch_id: Option<String>,
    pub message: String,
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

/// A score override supplied in the preview.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScoreInput {
    pub question_id: String,
    pub score: f64,
}

/// What a confirmation token carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingReadAloud {
    job_id: String,
    course_id: String,
    total_score: f64,
    row: Value,
}

struct LoadedTask {
    job: Job,
    paper: ReadAloudPaper,
    content_id: String,
    book_id: String,
    paper_id: String,
    course_id: String,
    assets: HashMap<String, String>,
}

/// Read the task's read-aloud slots, detected from its paper.
#[tauri::command(rename_all = "snake_case")]
pub async fn readaloud_get(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudTaskView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_task(&state, &account_id, &client, &job_id).await?;
    let submitted = is_submitted(
        &state,
        &account_id,
        &client,
        &job_id,
        &loaded.course_id,
        &loaded.job,
    )
    .await?;
    let expired = loaded
        .job
        .end_time
        .as_deref()
        .and_then(up366_core::parse_deadline)
        .is_some_and(|deadline| deadline < Utc::now());

    Ok(ReadAloudTaskView {
        job_id,
        title: loaded.job.title().to_owned(),
        submitted,
        expired,
        question_count: loaded.paper.questions.len(),
        sentence_count: loaded.paper.sentence_count,
        total_score: loaded.paper.total_score,
        slots: loaded
            .paper
            .questions
            .iter()
            .map(|question| ReadAloudSlotView {
                question_id: question.question_id.clone(),
                number: question.number,
                sentence: question.sentences.join(" "),
                sentences: question.sentences.clone(),
                score: question.score,
                duration: question.duration,
                audio_url: question.audio_url.clone(),
                record_url: loaded.assets.get(&question.question_id).cloned(),
            })
            .collect(),
    })
}

/// Validate one mp3, upload it to OSS, and remember the URL.
#[tauri::command(rename_all = "snake_case")]
pub async fn readaloud_upload(
    account_id: String,
    job_id: String,
    recording: RecordingUpload,
    state: tauri::State<'_, AppState>,
) -> CommandResult<UploadView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    if recording.question_id.trim().is_empty() {
        return Err(AppError::new("invalid_input", "缺少题目编号"));
    }

    let path = std::path::Path::new(&recording.path);
    let info = up366_media::inspect_mp3(path)
        .map_err(|error| AppError::new("invalid_input", format!("音频文件不可用: {error}")))?;

    let account = state
        .db
        .accounts()
        .get(&account_id)
        .await
        .map_err(storage_error)?
        .ok_or_else(AppError::auth_required)?;
    // `school_id` and `uid` are not persisted at login; the profile endpoint is
    // the authoritative source for both.
    let user = adapters::current_user(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let uid = account
        .uid
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(user.uid);
    let user_type = account
        .user_type
        .or_else(|| client.user_type())
        .ok_or_else(|| AppError::new("protocol_drift", "账号缺少 userType"))?;
    let school_id = account
        .school_id
        .or(user.school_id)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("protocol_drift", "账号缺少 organId"))?;

    // A uuid keeps two uploads with the same local file name from overwriting
    // each other under the same uid and date.
    let extension = recording
        .file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| !extension.is_empty())
        .unwrap_or("mp3");
    let object_file = format!("{}.{extension}", uuid::Uuid::new_v4().simple());
    let object_name = up366_media::object_name(user_type, &uid, &object_file, chrono::Local::now());
    let ticket = adapters::upload_ticket(&client, &school_id, &object_name)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| AppError::new("invalid_input", format!("读取音频失败: {error}")))?;
    adapters::upload_object(&ticket, bytes)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let record_url = ticket.url();

    state
        .db
        .audio()
        .upsert(
            &account_id,
            &up366_store::NewAudioAsset {
                asset_id: format!("{job_id}:{}", recording.question_id),
                job_id: job_id.clone(),
                question_id: recording.question_id.clone(),
                local_path: recording.path,
                object_name,
                object_url: record_url.clone(),
                size: i64::try_from(info.size).unwrap_or(i64::MAX),
                sha256: info.sha256,
                duration_ms: None,
            },
        )
        .await
        .map_err(storage_error)?;

    Ok(UploadView {
        ok: true,
        question_id: recording.question_id,
        file_name: recording.file_name,
        size: info.size,
        record_url,
    })
}

/// Build the exact score row and issue a single-use confirmation token.
#[tauri::command(rename_all = "snake_case")]
pub async fn readaloud_preview(
    account_id: String,
    job_id: String,
    scores: Option<Vec<ScoreInput>>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudPreviewView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_task(&state, &account_id, &client, &job_id).await?;
    if is_submitted(
        &state,
        &account_id,
        &client,
        &job_id,
        &loaded.course_id,
        &loaded.job,
    )
    .await?
    {
        return Err(AppError::new(
            "duplicate_submission",
            "该作业已提交，不能重复提交",
        ));
    }

    let mut recordings = Vec::with_capacity(loaded.paper.questions.len());
    let mut slots = Vec::with_capacity(loaded.paper.questions.len());
    let mut final_scores = Vec::with_capacity(loaded.paper.questions.len());
    for question in &loaded.paper.questions {
        let record_url = loaded
            .assets
            .get(&question.question_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "invalid_input",
                    format!("第 {} 题还没有上传录音", question.number),
                )
            })?;
        let score = scores
            .as_ref()
            .and_then(|list| {
                list.iter()
                    .find(|input| input.question_id == question.question_id)
                    .map(|input| input.score)
            })
            .unwrap_or(question.score);
        if !(0.0..=question.score).contains(&score) {
            return Err(AppError::new(
                "invalid_input",
                format!(
                    "第 {} 题分数需在 0 到 {} 之间",
                    question.number, question.score
                ),
            ));
        }
        recordings.push(ReadAloudRecording {
            question_id: question.question_id.clone(),
            record_url: record_url.clone(),
        });
        final_scores.push(score);
        slots.push(PreviewSlotView {
            question_id: question.question_id.clone(),
            number: question.number,
            sentence: question.sentences.join(" "),
            score,
            record_url,
        });
    }

    let uid: i64 = crate::commands::uid_for(&state, &account_id, &client)
        .await?
        .parse()
        .map_err(|_| AppError::new("protocol_drift", "uid 不是数字"))?;
    let context = ReadAloudContext {
        uid,
        client_id: client.client_id().to_owned(),
        course_id: loaded.course_id.clone(),
        book_id: loaded.book_id.clone(),
        content_id: loaded.content_id.clone(),
        job_id: job_id.clone(),
        paper_id: loaded.paper_id.clone(),
    };
    let study_date = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let seconds: i64 = loaded
        .paper
        .questions
        .iter()
        .map(|question| i64::from(question.duration))
        .sum();
    let row = adapters::build_row(
        &loaded.paper,
        &context,
        &recordings,
        &final_scores,
        study_date,
        seconds,
    )
    .map_err(AppError::from)?;
    let reported_score = final_scores.iter().sum();

    let pending = PendingReadAloud {
        job_id: job_id.clone(),
        course_id: loaded.course_id.clone(),
        total_score: loaded.paper.total_score,
        row: row.clone(),
    };
    let payload = serde_json::to_value(&pending)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id,
        operation: "readaloud_submit",
        payload,
    });

    Ok(ReadAloudPreviewView {
        job_id,
        title: loaded.job.title().to_owned(),
        total_score: loaded.paper.total_score,
        reported_score,
        row,
        slots,
        confirm_token,
    })
}

/// Submit a previewed row. The token is single-use.
#[tauri::command(rename_all = "snake_case")]
pub async fn readaloud_submit(
    confirm_token: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudSubmitView> {
    let (account_id, client) = state.require_client(None)?;
    let pending = state
        .confirmations
        .consume(&account_id, &confirm_token)
        .map_err(|failure| match failure {
            crate::confirm::Write::ConfirmationExpired => {
                AppError::new("confirmation_expired", "确认已过期或已使用，请重新预览")
            }
            crate::confirm::Write::AccountMismatch => {
                AppError::new("account_mismatch", "确认令牌不属于当前账号")
            }
        })?;
    let pending: PendingReadAloud = serde_json::from_value(pending.payload)
        .map_err(|error| AppError::new("protocol_drift", error.to_string()))?;

    adapters::submit_readaloud(&client, &pending.row, client.now_ms())
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    let (verified, score, batch_id) =
        verify_finished(&state, &account_id, &client, &pending).await?;

    Ok(ReadAloudSubmitView {
        ok: true,
        verified,
        score,
        batch_id,
        message: if verified {
            "提交成功".to_owned()
        } else {
            "服务端已返回成功，但完成列表尚未刷新，请稍后重新加载作业".to_owned()
        },
    })
}

/// Read the finished list back. The endpoint is account-wide, so one course id
/// is enough; a just-submitted row may take a moment to appear.
async fn verify_finished(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    pending: &PendingReadAloud,
) -> CommandResult<(bool, Option<f64>, Option<String>)> {
    for attempt in 0..4 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        let rows = match adapters::course_finished_jobs(client, &pending.course_id).await {
            Ok(rows) => rows,
            Err(error) if client.is_expired() => {
                return Err(state.session_error(account_id, client, error))
            }
            Err(_) => continue,
        };
        let Some(row) = rows
            .finished
            .iter()
            .find(|row| row.id() == Some(pending.job_id.as_str()))
        else {
            continue;
        };
        let score = row.score;
        let batch_id = row
            .raw
            .get("batchId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        state
            .db
            .cache()
            .upsert_job_status(
                account_id,
                &up366_store::JobStatusRecord {
                    job_id: JobId::from(pending.job_id.as_str()),
                    task_status: Some(1),
                    stu_status: Some(2),
                    submit_time: Some(up366_store::now_millis()),
                    score,
                    max_score: Some(pending.total_score),
                    fetched_at: up366_store::now_millis(),
                },
            )
            .await
            .map_err(storage_error)?;
        return Ok((true, score, batch_id));
    }
    Ok((false, None, None))
}

/// Load the job, its paper and any already-uploaded recordings.
async fn load_task(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    job_id: &str,
) -> CommandResult<LoadedTask> {
    let record = find_job(state, account_id, client, job_id).await?;
    let data: Value = serde_json::from_str(&record.raw_json)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let job = Job::from_value(data).map_err(AppError::from)?;

    let content_id = record
        .content_id
        .clone()
        .or_else(|| job.content().map(str::to_owned))
        .ok_or_else(|| AppError::new("protocol_drift", "作业缺少 contentId"))?;
    let book_id = record
        .book_id
        .clone()
        .or_else(|| job.book().map(str::to_owned))
        .unwrap_or_default();
    let course_id = record.course_id.as_str().to_owned();
    let paper_id = match record
        .paper_id
        .clone()
        .or_else(|| job.paper().map(str::to_owned))
    {
        Some(paper_id) if !paper_id.is_empty() => paper_id,
        _ => {
            let info = adapters::job_info(client, &course_id, job_id, &content_id, &book_id)
                .await
                .map_err(|error| state.session_error(account_id, client, error))?;
            info.paper_id
                .filter(|paper_id| !paper_id.is_empty())
                .ok_or_else(|| AppError::new("protocol_drift", "作业缺少 paperId"))?
        }
    };

    let detail = adapters::paper_detail(client, &paper_id)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    let path_prefix = detail
        .raw
        .get("pathPrefix")
        .and_then(Value::as_str)
        .unwrap_or("");
    let paper = adapters::parse_paper(&detail.paper_data, path_prefix).map_err(AppError::from)?;
    paper.require_all_read_aloud().map_err(AppError::from)?;

    let assets = state
        .db
        .audio()
        .for_job(account_id, job_id)
        .await
        .map_err(storage_error)?
        .into_iter()
        .filter_map(|asset| asset.object_url.map(|url| (asset.question_id, url)))
        .collect();

    Ok(LoadedTask {
        job,
        paper,
        content_id,
        book_id,
        paper_id,
        course_id,
        assets,
    })
}

/// Find a cached job, refreshing the catalog once when a direct link missed.
async fn find_job(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    job_id: &str,
) -> CommandResult<JobRecord> {
    let find = |rows: Vec<JobRecord>| rows.into_iter().find(|row| row.job_id.as_str() == job_id);
    if let Some(row) = find(
        state
            .db
            .cache()
            .jobs(account_id, None)
            .await
            .map_err(storage_error)?,
    ) {
        return Ok(row);
    }
    crate::commands::catalog::refresh_catalog(state, account_id, client).await?;
    find(
        state
            .db
            .cache()
            .jobs(account_id, None)
            .await
            .map_err(storage_error)?,
    )
    .ok_or_else(|| AppError::new("not_found", "作业不在缓存中，请先刷新作业列表"))
}

async fn is_submitted(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    job_id: &str,
    course_id: &str,
    job: &Job,
) -> CommandResult<bool> {
    if job.finished() {
        return Ok(true);
    }
    let status = state
        .db
        .cache()
        .job_status(account_id, &JobId::from(job_id))
        .await
        .map_err(storage_error)?;
    if status.is_some_and(|status| {
        status.task_status == Some(1) || status.stu_status.is_some_and(|value| value > 1)
    }) {
        return Ok(true);
    }
    match adapters::course_finished_jobs(client, course_id).await {
        Ok(rows) => Ok(rows.finished.iter().any(|row| row.id() == Some(job_id))),
        Err(error) if client.is_expired() => Err(state.session_error(account_id, client, error)),
        Err(_) => Ok(false),
    }
}

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_token_is_deserializable_and_keeps_the_study_task_id() {
        let pending = PendingReadAloud {
            job_id: "JOB-1".to_owned(),
            course_id: "COURSE-1".to_owned(),
            total_score: 30.0,
            row: serde_json::json!({ "studyTaskId": "JOB-1" }),
        };
        let value = serde_json::to_value(&pending).unwrap();
        let back: PendingReadAloud = serde_json::from_value(value).unwrap();
        assert_eq!(back.row["studyTaskId"], "JOB-1");
        assert_eq!(back.total_score, 30.0);
    }
}
