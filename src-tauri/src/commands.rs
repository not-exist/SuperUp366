//! Tauri command surface.
//!
//! One module per feature. `lib.rs` refers to commands by their full path so
//! each feature keeps its own namespace. Commands stay thin: read the active
//! account from state, call the protocol adapters, map to a view type.
//!
//! A command never builds an upstream request by hand and never claims a
//! success the server did not confirm.

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

/// The localized form of the server's `-13` ("already submitted / attempts
/// exhausted"). Submit commands use it so the UI does not show the protocol
/// layer's English wording; a write is never retried after this answer.
pub(crate) fn duplicate_submission_error() -> AppError {
    AppError::new(
        "duplicate_submission",
        "服务端拒绝重复提交：该作业已提交或作答次数已用完",
    )
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
