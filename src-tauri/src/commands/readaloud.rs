//! Read-aloud commands: detect slots, upload mp3s, preview, submit once.
//!
//! The flow follows the confirmed protocol (see
//! `docs/protocol/confirmed.md`):
//!
//! 1. `readaloud_get` resolves the job's paper and parses every recording
//!    element. The slot count is therefore **detected from the paper**, never
//!    guessed.
//! 2. `readaloud_upload` validates and uploads one mp3, storing the URL.
//! 3. `readaloud_preview` builds the exact score row and issues a single-use
//!    token.
//! 4. `readaloud_submit` consumes the token, posts once, then reads the
//!    completion list back.
//!
//! A class-assigned course book (`bookType = 2`) has assignments that are not
//! scheduled jobs: its pages come from `/client/books/tree`, the paper from
//! `xot-api /client/textbook/paperinfo`, and the submit row from the official
//! reader's "textbook" branch — empty `studyTaskId`, `submitType = 0`, and no
//! per-question `qstJson`. The same page/upload/preview/submit steps apply,
//! so both sources share the machinery below.
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
    /// The id this page was opened with: a job id, or a book task id.
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
    /// Whether the completion list already shows the task.
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

/// Where a previewed submission belongs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PendingTarget {
    /// A scheduled job: completion is keyed on the job id.
    Job { job_id: String, course_id: String },
    /// A course-book page: completion is keyed on the book task id.
    Book { book_id: String, task_id: String },
}

/// What a confirmation token carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingReadAloud {
    target: PendingTarget,
    total_score: f64,
    row: Value,
}

/// The two sources a read-aloud task can come from.
#[derive(Debug, Clone, PartialEq)]
enum TaskSource {
    /// A scheduled job from the account's job list.
    Job { job_id: String, course_id: String },
    /// A page of a class-assigned course book; it has no job row.
    Book { book_id: String, task_id: String },
}

struct LoadedTask {
    source: TaskSource,
    title: String,
    paper: ReadAloudPaper,
    /// `taskId` = `chapterId` = the job's content id, or the book page id.
    content_id: String,
    book_id: String,
    paper_id: String,
    course_id: String,
    expired: bool,
    submitted: bool,
    assets: HashMap<String, String>,
}

impl LoadedTask {
    /// The id the UI opened this task with.
    fn display_id(&self) -> String {
        match &self.source {
            TaskSource::Job { job_id, .. } => job_id.clone(),
            TaskSource::Book { task_id, .. } => task_id.clone(),
        }
    }
}

/// Read the task's read-aloud slots, detected from its paper.
#[tauri::command(rename_all = "snake_case")]
pub async fn readaloud_get(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudTaskView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_job_task(&state, &account_id, &client, &job_id).await?;
    Ok(task_view(&loaded))
}

/// Read a course-book page's read-aloud slots.
///
/// The page has no job row; its paper comes from the course book tree.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_readaloud_get(
    account_id: String,
    book_id: String,
    book_type: i64,
    course_id: String,
    task_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudTaskView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_book_task(
        &state,
        &account_id,
        &client,
        &book_id,
        book_type,
        &course_id,
        &task_id,
    )
    .await?;
    Ok(task_view(&loaded))
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
    upload_recording(&state, &account_id, &client, &job_id, recording).await
}

/// Upload one recording for a course-book page.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_readaloud_upload(
    account_id: String,
    book_id: String,
    task_id: String,
    recording: RecordingUpload,
    state: tauri::State<'_, AppState>,
) -> CommandResult<UploadView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let assets_key = book_assets_key(&book_id, &task_id);
    upload_recording(&state, &account_id, &client, &assets_key, recording).await
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
    let loaded = load_job_task(&state, &account_id, &client, &job_id).await?;
    prepare_preview(&state, &account_id, &client, loaded, scores).await
}

/// Build the score row for a course-book page and issue a token.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_readaloud_preview(
    account_id: String,
    book_id: String,
    book_type: i64,
    course_id: String,
    task_id: String,
    scores: Option<Vec<ScoreInput>>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<ReadAloudPreviewView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_book_task(
        &state,
        &account_id,
        &client,
        &book_id,
        book_type,
        &course_id,
        &task_id,
    )
    .await?;
    prepare_preview(&state, &account_id, &client, loaded, scores).await
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
        message: match (&pending.target, verified) {
            (_, true) => "提交成功".to_owned(),
            (PendingTarget::Book { .. }, false) => {
                "服务端已返回成功，但书本完成列表尚未刷新，请稍后重新加载书籍".to_owned()
            }
            (PendingTarget::Job { .. }, false) => {
                "服务端已返回成功，但完成列表尚未刷新，请稍后重新加载作业".to_owned()
            }
        },
    })
}

/// Read the completion list back. The endpoint is account-wide for jobs and
/// per-book for course books; a just-submitted row may take a moment to appear.
async fn verify_finished(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    pending: &PendingReadAloud,
) -> CommandResult<(bool, Option<f64>, Option<String>)> {
    match &pending.target {
        PendingTarget::Job { job_id, course_id } => {
            verify_finished_job(
                state,
                account_id,
                client,
                job_id,
                course_id,
                pending.total_score,
            )
            .await
        }
        PendingTarget::Book { book_id, task_id } => {
            verify_finished_book(state, account_id, client, book_id, task_id).await
        }
    }
}

async fn verify_finished_job(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    job_id: &str,
    course_id: &str,
    total_score: f64,
) -> CommandResult<(bool, Option<f64>, Option<String>)> {
    for attempt in 0..4 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        let rows = match adapters::course_finished_jobs(client, course_id).await {
            Ok(rows) => rows,
            Err(error) if client.is_expired() => {
                return Err(state.session_error(account_id, client, error))
            }
            Err(_) => continue,
        };
        let Some(row) = rows.finished.iter().find(|row| row.id() == Some(job_id)) else {
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
                    job_id: JobId::from(job_id),
                    task_status: Some(1),
                    stu_status: Some(2),
                    submit_time: Some(up366_store::now_millis()),
                    score,
                    max_score: Some(total_score),
                    fetched_at: up366_store::now_millis(),
                },
            )
            .await
            .map_err(storage_error)?;
        return Ok((true, score, batch_id));
    }
    Ok((false, None, None))
}

async fn verify_finished_book(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    book_id: &str,
    task_id: &str,
) -> CommandResult<(bool, Option<f64>, Option<String>)> {
    for attempt in 0..4 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        let rows = match adapters::finished_tasks(client, book_id).await {
            Ok(rows) => rows,
            Err(error) if client.is_expired() => {
                return Err(state.session_error(account_id, client, error))
            }
            Err(_) => continue,
        };
        let Some(row) = rows
            .iter()
            .find(|row| row.task_id.as_deref() == Some(task_id))
        else {
            continue;
        };
        return Ok((true, row.score, row.batch_id.clone()));
    }
    Ok((false, None, None))
}

/// Build the view from a loaded task. The displayed id is the id the page was
/// opened with, which is also what `readaloud_submit` verifies against.
fn task_view(loaded: &LoadedTask) -> ReadAloudTaskView {
    ReadAloudTaskView {
        job_id: loaded.display_id(),
        title: loaded.title.clone(),
        submitted: loaded.submitted,
        expired: loaded.expired,
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
    }
}

/// Validate one mp3, upload it to OSS, and remember the URL.
async fn upload_recording(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    assets_key: &str,
    recording: RecordingUpload,
) -> CommandResult<UploadView> {
    if recording.question_id.trim().is_empty() {
        return Err(AppError::new("invalid_input", "缺少题目编号"));
    }

    let path = std::path::Path::new(&recording.path);
    let info = up366_media::inspect_mp3(path)
        .map_err(|error| AppError::new("invalid_input", format!("音频文件不可用: {error}")))?;

    let account = state
        .db
        .accounts()
        .get(account_id)
        .await
        .map_err(storage_error)?
        .ok_or_else(AppError::auth_required)?;
    // `school_id` and `uid` are not persisted at login; the profile endpoint is
    // the authoritative source for both.
    let user = adapters::current_user(client)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
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
    let ticket = adapters::upload_ticket(client, &school_id, &object_name)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| AppError::new("invalid_input", format!("读取音频失败: {error}")))?;
    adapters::upload_object(&ticket, bytes)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    let record_url = ticket.url();

    state
        .db
        .audio()
        .upsert(
            account_id,
            &up366_store::NewAudioAsset {
                asset_id: format!("{assets_key}:{}", recording.question_id),
                job_id: assets_key.to_owned(),
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

/// Build the score row for a loaded task and issue a single-use token.
async fn prepare_preview(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    loaded: LoadedTask,
    scores: Option<Vec<ScoreInput>>,
) -> CommandResult<ReadAloudPreviewView> {
    if loaded.submitted {
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

    let uid: i64 = crate::commands::uid_for(state, account_id, client)
        .await?
        .parse()
        .map_err(|_| AppError::new("protocol_drift", "uid 不是数字"))?;
    let job_id = match &loaded.source {
        TaskSource::Job { job_id, .. } => Some(job_id.clone()),
        TaskSource::Book { .. } => None,
    };
    let context = ReadAloudContext {
        uid,
        client_id: client.client_id().to_owned(),
        course_id: loaded.course_id.clone(),
        book_id: loaded.book_id.clone(),
        content_id: loaded.content_id.clone(),
        job_id,
        paper_id: loaded.paper_id.clone(),
    };
    let study_date = adapters::server_time(client)
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

    let target = match &loaded.source {
        TaskSource::Job { job_id, course_id } => PendingTarget::Job {
            job_id: job_id.clone(),
            course_id: course_id.clone(),
        },
        TaskSource::Book { book_id, task_id } => PendingTarget::Book {
            book_id: book_id.clone(),
            task_id: task_id.clone(),
        },
    };
    let display_id = loaded.display_id();
    let pending = PendingReadAloud {
        target,
        total_score: loaded.paper.total_score,
        row: row.clone(),
    };
    let payload = serde_json::to_value(&pending)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: account_id.clone(),
        operation: "readaloud_submit",
        payload,
    });

    Ok(ReadAloudPreviewView {
        job_id: display_id,
        title: loaded.title,
        total_score: loaded.paper.total_score,
        reported_score,
        row,
        slots,
        confirm_token,
    })
}

/// Load a scheduled job, its paper and any already-uploaded recordings.
async fn load_job_task(
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

    let assets = load_assets(state, account_id, job_id).await?;
    let expired = job
        .end_time
        .as_deref()
        .and_then(up366_core::parse_deadline)
        .is_some_and(|deadline| deadline < Utc::now());
    let submitted = is_submitted(state, account_id, client, job_id, &course_id, &job).await?;

    Ok(LoadedTask {
        source: TaskSource::Job {
            job_id: job_id.to_owned(),
            course_id: course_id.clone(),
        },
        title: job.title().to_owned(),
        paper,
        content_id,
        book_id,
        paper_id,
        course_id,
        expired,
        submitted,
        assets,
    })
}

/// Load a course-book page, its paper and any already-uploaded recordings.
async fn load_book_task(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    book_id: &str,
    book_type: i64,
    course_id: &str,
    task_id: &str,
) -> CommandResult<LoadedTask> {
    // Only course books use the page/paper reader this path mirrors. An
    // ordinary book's tasks submit through a different (gzip) branch.
    if book_type != 2 {
        return Err(AppError::new(
            "unsupported",
            "只有班级课本（bookType=2）支持按书本任务提交",
        ));
    }
    if task_id.trim().is_empty() {
        return Err(AppError::new("invalid_input", "缺少书本任务编号"));
    }
    if course_id.trim().is_empty() {
        return Err(AppError::new("invalid_input", "缺少课程编号"));
    }

    let pages = adapters::book_tree_pages(client, course_id, book_id, book_type)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    let page = pages
        .iter()
        .find(|page| page.id == task_id)
        .ok_or_else(|| AppError::new("not_found", "该任务不在课本目录中"))?;
    if !page.is_content() {
        return Err(AppError::new("invalid_input", "该目录项不是可作答的作业"));
    }
    let paper_id = page
        .paper_id
        .clone()
        .filter(|paper_id| !paper_id.is_empty())
        .ok_or_else(|| AppError::new("protocol_drift", "书本任务缺少 paperId"))?;

    let info = adapters::textbook_paperinfo(client, &paper_id)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    let paper =
        adapters::parse_paper(&info.paper_data, info.path_prefix()).map_err(AppError::from)?;
    paper.require_all_read_aloud().map_err(AppError::from)?;

    let assets = load_assets(state, account_id, &book_assets_key(book_id, task_id)).await?;
    let submitted = adapters::finished_tasks(client, book_id)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?
        .iter()
        .any(|row| row.task_id.as_deref() == Some(task_id));

    Ok(LoadedTask {
        source: TaskSource::Book {
            book_id: book_id.to_owned(),
            task_id: task_id.to_owned(),
        },
        title: if page.name.is_empty() {
            "书本朗读".to_owned()
        } else {
            page.name.clone()
        },
        paper,
        content_id: task_id.to_owned(),
        book_id: book_id.to_owned(),
        paper_id,
        course_id: course_id.to_owned(),
        expired: false,
        submitted,
        assets,
    })
}

/// Recordings already uploaded for this task.
async fn load_assets(
    state: &AppState,
    account_id: &AccountId,
    assets_key: &str,
) -> CommandResult<HashMap<String, String>> {
    Ok(state
        .db
        .audio()
        .for_job(account_id, assets_key)
        .await
        .map_err(storage_error)?
        .into_iter()
        .filter_map(|asset| asset.object_url.map(|url| (asset.question_id, url)))
        .collect())
}

/// The storage key for a course-book page's recordings.
fn book_assets_key(book_id: &str, task_id: &str) -> String {
    format!("book:{book_id}:{task_id}")
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
            target: PendingTarget::Job {
                job_id: "JOB-1".to_owned(),
                course_id: "COURSE-1".to_owned(),
            },
            total_score: 30.0,
            row: serde_json::json!({ "studyTaskId": "JOB-1" }),
        };
        let value = serde_json::to_value(&pending).unwrap();
        let back: PendingReadAloud = serde_json::from_value(value).unwrap();
        assert_eq!(back.row["studyTaskId"], "JOB-1");
        assert_eq!(back.total_score, 30.0);
        assert!(matches!(
            back.target,
            PendingTarget::Job { ref job_id, .. } if job_id == "JOB-1"
        ));
    }

    #[test]
    fn a_book_token_round_trips_without_a_job_id() {
        let pending = PendingReadAloud {
            target: PendingTarget::Book {
                book_id: "BOOK-1".to_owned(),
                task_id: "TASK-1".to_owned(),
            },
            total_score: 100.0,
            row: serde_json::json!({ "studyTaskId": "", "submitType": 0 }),
        };
        let back: PendingReadAloud =
            serde_json::from_value(serde_json::to_value(&pending).unwrap()).unwrap();
        assert!(matches!(
            back.target,
            PendingTarget::Book { ref task_id, .. } if task_id == "TASK-1"
        ));
        assert_eq!(back.row["submitType"], 0);
    }

    #[test]
    fn a_book_asset_key_is_namespaced_by_book_and_task() {
        assert_eq!(book_assets_key("BOOK-1", "TASK-1"), "book:BOOK-1:TASK-1");
    }
}
