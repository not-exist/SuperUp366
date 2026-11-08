//! Homework commands: the per-question answer view.

use serde::Serialize;
use up366_core::AccountId;
use up366_protocol::{adapters, dto::AnswerDetail, AnswerDetailParams};
use up366_store::AccountRecord;

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// Fields the frontend renders for one question.
///
/// `student_answer` and `correct_answer` are the XML fragments from the answer
/// endpoint, passed through untouched so rendering never loses information.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnswerQuestionView {
    pub question_id: String,
    pub number: usize,
    /// Upstream question type marker, e.g. `9` for read-aloud.
    pub question_type: String,
    pub stem: String,
    pub student_answer: String,
    pub correct_answer: String,
    pub max_score: Option<f64>,
    pub score: Option<f64>,
    /// Whether this slot needs an uploaded recording.
    pub read_aloud: bool,
}

/// Answer document for one job.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnswerDocumentView {
    pub job_id: String,
    /// `contentId` — the id the upstream endpoint requires.
    pub task_id: String,
    pub title: String,
    pub score: Option<f64>,
    pub questions: Vec<AnswerQuestionView>,
    /// Raw paper XML, rendered collapsed in the UI.
    pub paper_xml: String,
}

/// The ids the answer endpoint needs for one job.
#[derive(Debug, Clone, PartialEq)]
struct JobIds {
    course_id: String,
    book_id: String,
    content_id: String,
}

/// Fetch and flatten the answer detail for one job.
///
/// The ids come from the cached job row and the stored account, so the front
/// end never has to know the `contentId` rule: the job id is never what the
/// endpoint wants.
#[tauri::command(rename_all = "snake_case")]
pub async fn answer_get(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AnswerDocumentView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let account = load_account(&state, &account_id).await?;
    let ids = find_job_ids(&state, &account_id, &job_id).await?;

    let uid = account.uid.clone().ok_or_else(AppError::auth_required)?;
    let params = AnswerDetailParams::new(
        ids.course_id.clone(),
        uid,
        ids.book_id.clone(),
        ids.content_id.clone(),
    );

    let detail = adapters::answer_detail(&client, &params)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    Ok(build_view(job_id, ids.content_id, &detail))
}

/// Read the stored account, required for `uid`.
async fn load_account(state: &AppState, account_id: &AccountId) -> CommandResult<AccountRecord> {
    state
        .db
        .accounts()
        .get(account_id)
        .await
        .map_err(storage_error)?
        .ok_or_else(AppError::auth_required)
}

/// Locate a cached job and return the ids the endpoint requires.
///
/// A miss means the job list was never fetched (or the cache was cleared), so
/// the caller must refresh rather than guess ids.
async fn find_job_ids(
    state: &AppState,
    account_id: &AccountId,
    job_id: &str,
) -> CommandResult<JobIds> {
    let rows = state
        .db
        .cache()
        .jobs(account_id, None)
        .await
        .map_err(storage_error)?;
    let row = rows
        .into_iter()
        .find(|row| row.job_id.as_str() == job_id)
        .ok_or_else(|| AppError::new("not_found", "作业不在缓存中，请先刷新作业列表"))?;
    Ok(JobIds {
        course_id: row.course_id.to_string(),
        book_id: row.book_id.map(|id| id.to_string()).unwrap_or_default(),
        content_id: row
            .content_id
            .clone()
            .ok_or_else(|| AppError::new("protocol_drift", "作业缺少 contentId"))?,
    })
}

/// Build the view from the decoded endpoint payload.
fn build_view(job_id: String, task_id: String, detail: &AnswerDetail) -> AnswerDocumentView {
    let paper_xml = detail.paper_data().unwrap_or_default();
    AnswerDocumentView {
        job_id,
        task_id,
        title: detail.task_name.clone().unwrap_or_default(),
        score: detail.score(),
        questions: Vec::new(),
        paper_xml: paper_xml.to_owned(),
    }
}

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> AnswerDetail {
        serde_json::from_value(serde_json::json!({
            "paperInfo": { "paperData": "<elements/>", "score": 30 },
            "userAnswer": { "answerData": "<answers/>" },
            "taskName": "Unit 4"
        }))
        .unwrap()
    }

    #[test]
    fn build_view_keeps_the_task_id_the_endpoint_requires() {
        let view = build_view("JOB-1".to_owned(), "CONTENT-1".to_owned(), &detail());
        assert_eq!(view.task_id, "CONTENT-1");
        assert_ne!(view.task_id, view.job_id);
        assert_eq!(view.paper_xml, "<elements/>");
        assert_eq!(view.score, Some(30.0));
    }

    #[test]
    fn a_missing_paper_leaves_an_empty_xml_not_a_failure() {
        let detail: AnswerDetail =
            serde_json::from_value(serde_json::json!({"userAnswer": {}})).unwrap();
        let view = build_view("JOB-2".to_owned(), "CONTENT-2".to_owned(), &detail);
        assert!(view.paper_xml.is_empty());
        assert_eq!(view.score, None);
    }
}
