//! Tauri command surface.
//!
//! One module per feature. `lib.rs` refers to commands by their full path so
//! each feature keeps its own namespace. Commands stay thin: read the active
//! account from state, call the protocol adapters, map to a view type.
//!
//! A command never builds an upstream request by hand and never claims a
//! success the server did not confirm.

use chrono::Utc;
use serde_json::Value;
use up366_core::{AccountId, JobId};
use up366_protocol::dto::Job;
use up366_protocol::{adapters, Client};
use up366_store::JobRecord;

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

pub mod accounts;
pub mod books;
pub mod catalog;
pub mod correction;
pub mod dictation;
pub mod homework;
pub mod readaloud;
pub mod system;
pub mod wordpk;
pub mod writing;

/// The numeric student id the endpoints expect.
///
/// The local account id is the lower-cased username, not a uid. The stored
/// profile usually has it; otherwise the user API is asked once per call.
pub(crate) async fn uid_for(
    state: &AppState,
    account_id: &AccountId,
    client: &Client,
) -> CommandResult<String> {
    if let Some(account) = state
        .db
        .accounts()
        .get(account_id)
        .await
        .map_err(|error| AppError::new("storage_error", error.to_string()))?
    {
        if let Some(uid) = account.uid.filter(|uid| !uid.trim().is_empty()) {
            return Ok(uid);
        }
    }
    up366_protocol::adapters::current_user(client)
        .await
        .map(|user| user.uid)
        .map_err(|error| state.session_error(account_id, client, error))
}

/// Find a cached job, refreshing the catalog once when a direct link missed.
///
/// Both the read-aloud and applied-writing pages open from a job id, and a
/// direct link may arrive before the list has ever been fetched.
pub(crate) async fn find_job(
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

/// Whether the account has already submitted this job.
///
/// The job's own status is checked first, then the status cache, and finally
/// the account-wide finished list. A failed list read is not fatal: the page
/// only uses this to disable the submit button.
pub(crate) async fn is_job_submitted(
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

/// The ids and paper XML one job's answer page needs.
///
/// Every page that renders a paper (applied writing, dictation) resolves the
/// same way: the job row may lack `contentId`/`paperId`, and `job/info` is the
/// fallback. Keeping it here stops the two loaders from drifting apart.
pub(crate) struct JobPaperContext {
    pub job_id: String,
    pub course_id: String,
    pub title: String,
    pub paper_id: String,
    pub content_id: String,
    pub paper_xml: String,
    pub submitted: bool,
    pub expired: bool,
}

pub(crate) async fn load_paper_context(
    state: &AppState,
    account_id: &AccountId,
    client: &Client,
    job_id: &str,
) -> CommandResult<JobPaperContext> {
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
    let submitted = is_job_submitted(state, account_id, client, job_id, &course_id, &job).await?;
    let expired = job
        .end_time
        .as_deref()
        .and_then(up366_core::parse_deadline)
        .is_some_and(|deadline| deadline < Utc::now());

    Ok(JobPaperContext {
        job_id: job_id.to_owned(),
        course_id,
        title: job.title().to_owned(),
        paper_id,
        content_id,
        paper_xml: detail.paper_data,
        submitted,
        expired,
    })
}

/// Whether a submitted job may be redone, and why not when it may not.
///
/// A probe failure that is not a session failure is reported as "not
/// supported" with a reason: the page must stay read-only when the answer is
/// unknown.
pub(crate) async fn retest_state(
    state: &AppState,
    account_id: &AccountId,
    client: &Client,
    content_id: &str,
    submitted: bool,
) -> CommandResult<(bool, String)> {
    if !submitted {
        return Ok((false, String::new()));
    }
    let uid: i64 = uid_for(state, account_id, client)
        .await?
        .parse()
        .map_err(|_| AppError::new("protocol_drift", "uid 不是数字"))?;
    match adapters::probe_retest(client, content_id, uid).await {
        Ok(support) => Ok((support.supported, support.reason)),
        Err(error) if client.is_expired() => Err(state.session_error(account_id, client, error)),
        Err(_) => Ok((false, "无法检测重做状态，请稍后刷新".to_owned())),
    }
}

/// Read the finished list back after a submission.
///
/// A just-submitted row may take a moment to appear, so the list is polled a
/// few times. A hit also refreshes the local status cache. The returned tuple
/// is `(verified, score, batch_id)`.
pub(crate) async fn verify_finished_job(
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

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}
