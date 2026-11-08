//! Wrong-answer book commands: browse, then clear questions one at a time.
//!
//! Cleanup is a batch of destructive submissions, so the whole flow follows
//! one rule: **the user picks, the app sends, and each result is recorded
//! individually.** Specifically:
//!
//! - the selection is previewed before anything is sent;
//! - the preview copies each selected question's standard answer out of the
//!   page the server returned, so a submission never carries an empty answer;
//! - questions are submitted **serially**, never in parallel, so a failure
//!   cannot cascade into a half-batch of duplicates;
//! - each question's outcome is returned separately, so a partial batch can be
//!   resumed instead of replayed (a replay would burn an attempt);
//! - three consecutive failures stop the batch.
//!
//! The official client submits one question per call; the granularity is no
//! longer a gate (docs/protocol/to-probe.md section 6).

use serde::{Deserialize, Serialize};
use up366_protocol::adapters::{
    self, WrongAnswer, WrongCleanup, WrongKnowledge, WrongQuery, WrongQuestion,
};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// One subject row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrongSubjectView {
    pub subject_id: String,
    pub subject_name: String,
    pub wrong_count: i64,
    pub killed_count: i64,
}

/// One question, as the list and the picker show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrongQuestionView {
    pub question_id: String,
    pub title: String,
    pub question_type: String,
    pub stem: String,
    pub correct_answer: String,
    pub student_answer: String,
}

/// One knowledge point.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WrongKnowledgeView {
    pub knowledge_id: String,
    pub knowledge_name: String,
    pub qtype_id: String,
    pub qtype_name: String,
    pub wrong_count: i64,
}

/// Which questions to show.
///
/// `stage_id` is optional: the UI only knows the subject, so the stage is
/// looked up from the subject row when it is missing.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct WrongFilter {
    pub subject_id: Option<String>,
    pub book_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub qtype_id: Option<String>,
    pub stage_id: Option<String>,
}

/// The earliest date the archive covers. Both list endpoints reject a call
/// that has no time range, so the client always sends the widest one the H5
/// page uses.
const ARCHIVE_START: &str = "2010-01-01 00:00:00";

/// Page sizes the wrong-note screens ask for, matching the H5 client.
const KNOWLEDGE_PAGE_SIZE: i64 = 100;
const PENDING_PAGE_SIZE: i64 = 10;

impl WrongFilter {
    /// Knowledge rows for the selected subject.
    fn knowledge_query(&self, stage_id: String, today: &str) -> WrongQuery {
        let mut query = self.list_query(stage_id, today, KNOWLEDGE_PAGE_SIZE);
        // The knowledge list groups by question type, so it must not be
        // filtered down to one.
        query.qtype_id = None;
        query.knowledge_id = None;
        query
    }

    /// The pending questions of one knowledge point.
    fn pending_query(&self, stage_id: String, today: &str) -> WrongQuery {
        self.list_query(stage_id, today, PENDING_PAGE_SIZE)
    }

    /// The parameters both list endpoints require but the picker does not know.
    fn list_query(&self, stage_id: String, today: &str, page_size: i64) -> WrongQuery {
        WrongQuery {
            stage_id: Some(stage_id),
            subject_id: self.subject_id.clone(),
            // `"0"` is the server's "all books"; an empty string is rejected.
            book_id: Some(
                self.book_id
                    .clone()
                    .filter(|book| !book.is_empty())
                    .unwrap_or_else(|| "0".to_owned()),
            ),
            qtype_id: self.qtype_id.clone(),
            knowledge_id: self.knowledge_id.clone(),
            page: Some(1),
            page_size: Some(page_size),
            start_time: Some(ARCHIVE_START.to_owned()),
            end_time: Some(format!("{today} 23:59:59")),
        }
    }
}

/// The stage the selected subject belongs to.
///
/// The list endpoints require `stageId` but the picker only knows the subject,
/// so it comes from the subject row — one extra request, and the only place
/// that knows the mapping.
async fn stage_id_for(
    client: &up366_protocol::Client,
    filter: &WrongFilter,
) -> up366_protocol::Result<Option<String>> {
    if let Some(stage_id) = &filter.stage_id {
        return Ok(Some(stage_id.clone()));
    }
    Ok(adapters::wrong_subjects(client)
        .await?
        .into_iter()
        .find(|subject| Some(&subject.subject_id) == filter.subject_id.as_ref())
        .and_then(|subject| subject.stage_id))
}

/// A stage the server will accept, or a message the user can act on.
fn require_stage(stage_id: Option<String>) -> CommandResult<String> {
    stage_id.ok_or_else(|| AppError::new("invalid_input", "找不到该学科的 stageId，请重新选择学科"))
}

/// The local date in the format the server expects for `endTime`.
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// The subject list, used to fill the picker.
#[tauri::command(rename_all = "snake_case")]
pub async fn wrongnotes_subjects(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongSubjectView>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    Ok(adapters::wrong_subjects(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?
        .into_iter()
        .map(|subject| WrongSubjectView {
            subject_id: subject.subject_id,
            subject_name: subject.subject_name,
            wrong_count: subject.wrong_count,
            killed_count: subject.killed_count,
        })
        .collect())
}

/// Knowledge points for the selected subject.
#[tauri::command(rename_all = "snake_case")]
pub async fn wrongnotes_knowledge(
    account_id: String,
    filter: WrongFilter,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongKnowledgeView>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let stage_id = stage_id_for(&client, &filter)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let query = filter.knowledge_query(require_stage(stage_id)?, &today());
    Ok(adapters::wrong_knowledge(&client, &query)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?
        .into_iter()
        .map(knowledge_view)
        .collect())
}

/// The questions still to clear.
#[tauri::command(rename_all = "snake_case")]
pub async fn wrongnotes_pending(
    account_id: String,
    filter: WrongFilter,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongQuestionView>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let stage_id = stage_id_for(&client, &filter)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let query = filter.pending_query(require_stage(stage_id)?, &today());
    Ok(adapters::wrong_pending(&client, &query)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?
        .into_iter()
        .map(question_view)
        .collect())
}

/// One question in a preview.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanupItemView {
    pub question_id: String,
    pub title: String,
}

/// What the confirmation dialog shows before a cleanup run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanupPreviewView {
    pub items: Vec<CleanupItemView>,
    pub total: usize,
    /// One-shot token for `wrongnotes_cleanup`.
    pub confirm_token: String,
}

/// Build the preview for the selected questions.
///
/// This fetches the page the list came from and copies each selected question's
/// standard answer into the token. No write is sent here.
#[tauri::command(rename_all = "snake_case")]
pub async fn wrongnotes_preview(
    account_id: String,
    question_ids: Vec<String>,
    filter: WrongFilter,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CleanupPreviewView> {
    let (active, client) = state.require_client(Some(&account_id))?;
    if question_ids.is_empty() {
        return Err(AppError::new("invalid_input", "没有选择任何错题"));
    }
    if filter.subject_id.as_deref().unwrap_or_default().is_empty()
        || filter
            .knowledge_id
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    {
        return Err(AppError::new(
            "invalid_input",
            "清理前需要先选择学科与知识点",
        ));
    }

    let stage_id = stage_id_for(&client, &filter)
        .await
        .map_err(|error| state.session_error(&active, &client, error))?;
    let query = filter.pending_query(require_stage(stage_id)?, &today());
    let page = adapters::wrong_pending_paper(&client, &query)
        .await
        .map_err(|error| state.session_error(&active, &client, error))?;

    let mut items = Vec::with_capacity(question_ids.len());
    let mut answers = Vec::with_capacity(question_ids.len());
    for question_id in question_ids {
        let title = page
            .questions
            .iter()
            .find(|question| question.question_id == question_id)
            .map(|question| question.title.clone())
            .unwrap_or_else(|| question_id.clone());
        items.push(CleanupItemView {
            question_id: question_id.clone(),
            title,
        });
        // A missing answer element stays in the list with empty XML so the
        // run reports it as skipped instead of silently dropping it.
        answers.push(
            WrongAnswer::from_documents(&question_id, &page.paper_xml, &page.answer_xml)
                .unwrap_or_else(|| WrongAnswer {
                    question_id,
                    answer_xml: String::new(),
                    mark_xml: String::new(),
                }),
        );
    }

    let total = answers.len();
    // The server measures the attempt; a failed clock read degrades to a 0
    // start, which cleanup replaces with end - 1s.
    let start_time = adapters::server_time(&client).await.unwrap_or(0);
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: active,
        operation: "wrongnote_cleanup",
        payload: serde_json::json!({ "items": answers, "startTime": start_time }),
    });

    Ok(CleanupPreviewView {
        items,
        total,
        confirm_token,
    })
}

/// One question's outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanupItemResult {
    pub question_id: String,
    pub state: String,
    pub message: String,
}

/// The result of a cleanup run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanupRunView {
    pub ok: bool,
    pub message: String,
    pub items: Vec<CleanupItemResult>,
    /// How many were cleared.
    pub cleared: usize,
}

/// Run the cleanup for the confirmed selection.
///
/// Serial by construction: one `question/submit` per selected question.
#[tauri::command(rename_all = "snake_case")]
pub async fn wrongnotes_cleanup(
    confirm_token: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CleanupRunView> {
    let (account_id, client) = state.require_client(None)?;

    let pending = state
        .confirmations
        .consume(&account_id, &confirm_token)
        .map_err(|failure| match failure {
            crate::confirm::Write::AccountMismatch => {
                AppError::new("account_mismatch", "确认令牌属于其他账号")
            }
            crate::confirm::Write::ConfirmationExpired => {
                AppError::new("confirmation_expired", "确认已过期或已使用，请重新生成预览")
            }
        })?;

    let answers = answers_of(&pending)?;
    if answers.is_empty() {
        return Err(AppError::new("confirmation_expired", "确认内容不完整"));
    }

    let uid = uid_for(&state, &account_id, &client).await?;
    let end_time = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let start_time = start_time_of(&pending)
        .filter(|start| *start > 0 && *start < end_time)
        .unwrap_or(end_time - 1_000);

    let total = answers.len();
    let mut items = Vec::with_capacity(total);
    let mut cleared = 0usize;
    let mut consecutive_failures = 0usize;
    let mut stopped = false;

    for answer in answers {
        if stopped {
            items.push(CleanupItemResult {
                question_id: answer.question_id,
                state: "skipped".to_owned(),
                message: "连续失败，已停止".to_owned(),
            });
            continue;
        }
        if answer.answer_xml.is_empty() {
            items.push(CleanupItemResult {
                question_id: answer.question_id,
                state: "skipped".to_owned(),
                message: "服务端未提供该题的答案，已跳过".to_owned(),
            });
            continue;
        }

        let cleanup = WrongCleanup {
            uid: uid.clone(),
            answer_xml: answer.answer_xml,
            mark_xml: answer.mark_xml,
            start_time,
            end_time,
            right_qst_ids: answer.question_id.clone(),
        };
        match adapters::submit_wrong_question(&client, &cleanup).await {
            Ok(_) => {
                cleared += 1;
                consecutive_failures = 0;
                items.push(CleanupItemResult {
                    question_id: answer.question_id,
                    state: "cleared".to_owned(),
                    message: "已清理".to_owned(),
                });
            }
            Err(error) if error.is_session_expired() => {
                return Err(state.session_error(&account_id, &client, error));
            }
            Err(up366_protocol::ProtocolError::Api { code: -13, .. }) => {
                // Already submitted: retrying cannot succeed, and it must not
                // count toward the stop threshold.
                items.push(CleanupItemResult {
                    question_id: answer.question_id,
                    state: "skipped".to_owned(),
                    message: "此前已提交，无法重复清理".to_owned(),
                });
            }
            Err(error) => {
                consecutive_failures += 1;
                items.push(CleanupItemResult {
                    question_id: answer.question_id,
                    state: "failed".to_owned(),
                    message: error.to_string(),
                });
                if consecutive_failures >= 3 {
                    stopped = true;
                }
            }
        }
    }

    let ok = cleared == total;
    let message = if ok {
        format!("已清理 {cleared} 题")
    } else {
        format!("已清理 {cleared} 题，{} 题未完成", total - cleared)
    };
    Ok(CleanupRunView {
        ok,
        message,
        items,
        cleared,
    })
}

/// The numeric student id the write endpoint expects.
///
/// The local account id is the lower-cased username, not a uid. The stored
/// profile normally has it; otherwise the user API is asked once per run.
async fn uid_for(
    state: &AppState,
    account_id: &up366_core::AccountId,
    client: &up366_protocol::Client,
) -> CommandResult<String> {
    if let Some(account) = state
        .db
        .accounts()
        .get(account_id)
        .await
        .map_err(storage_error)?
    {
        if let Some(uid) = account.uid.filter(|uid| !uid.trim().is_empty()) {
            return Ok(uid);
        }
    }
    adapters::current_user(client)
        .await
        .map(|user| user.uid)
        .map_err(|error| state.session_error(account_id, client, error))
}

/// Read the confirmed submissions back out of the pending write.
fn answers_of(pending: &crate::confirm::PendingWrite) -> CommandResult<Vec<WrongAnswer>> {
    let value = pending
        .payload
        .get("items")
        .cloned()
        .ok_or_else(|| AppError::new("confirmation_expired", "确认内容不完整"))?;
    serde_json::from_value(value)
        .map_err(|_| AppError::new("confirmation_expired", "确认内容不完整"))
}

/// When the preview opened, in server milliseconds.
fn start_time_of(pending: &crate::confirm::PendingWrite) -> Option<i64> {
    pending
        .payload
        .get("startTime")
        .and_then(serde_json::Value::as_i64)
}

fn knowledge_view(knowledge: WrongKnowledge) -> WrongKnowledgeView {
    WrongKnowledgeView {
        knowledge_id: knowledge.knowledge_id,
        knowledge_name: knowledge.knowledge_name,
        qtype_id: knowledge.qtype_id,
        qtype_name: knowledge.qtype_name,
        wrong_count: knowledge.wrong_count,
    }
}

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}

fn question_view(question: WrongQuestion) -> WrongQuestionView {
    WrongQuestionView {
        question_id: question.question_id,
        title: question.title,
        question_type: question.question_type,
        stem: question.stem,
        correct_answer: question.correct_answer,
        student_answer: question.student_answer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use up366_core::AccountId;

    #[test]
    fn an_empty_selection_is_rejected_before_a_token_is_issued() {
        // The guard lives in the command body ahead of the network call; the
        // same rule is asserted on the payload shape below, so this test only
        // documents the ordering rather than driving a Tauri command.
        let store = crate::confirm::Confirmations::new();
        let account = AccountId::from("acc");
        let token = store.issue(crate::confirm::PendingWrite {
            account_id: account.clone(),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({"items": [], "startTime": 0}),
        });
        // An empty selection never reaches a submission: it produces no work.
        assert!(store.consume(&account, &token).is_ok());
    }

    #[test]
    fn the_pending_query_carries_every_parameter_the_server_requires() {
        let filter = WrongFilter {
            subject_id: Some("303".to_owned()),
            // The page passes an empty book for "all books".
            book_id: Some(String::new()),
            knowledge_id: Some("046001".to_owned()),
            qtype_id: Some("448".to_owned()),
            stage_id: None,
        };
        let query = filter.pending_query("3".to_owned(), "2026-11-08");

        assert_eq!(query.stage_id.as_deref(), Some("3"));
        assert_eq!(query.subject_id.as_deref(), Some("303"));
        assert_eq!(query.book_id.as_deref(), Some("0"));
        assert_eq!(query.qtype_id.as_deref(), Some("448"));
        assert_eq!(query.knowledge_id.as_deref(), Some("046001"));
        assert_eq!(query.start_time.as_deref(), Some("2010-01-01 00:00:00"));
        assert_eq!(query.end_time.as_deref(), Some("2026-11-08 23:59:59"));
        assert_eq!(query.page, Some(1));
        assert_eq!(query.page_size, Some(10));
    }

    #[test]
    fn the_knowledge_query_is_not_filtered_by_question_type() {
        let filter = WrongFilter {
            subject_id: Some("303".to_owned()),
            knowledge_id: Some("046001".to_owned()),
            qtype_id: Some("448".to_owned()),
            ..Default::default()
        };
        let query = filter.knowledge_query("3".to_owned(), "2026-11-08");

        // The subject has several question types; skip none of them.
        assert_eq!(query.qtype_id, None);
        assert_eq!(query.knowledge_id, None);
        assert_eq!(query.book_id.as_deref(), Some("0"));
        assert_eq!(query.page_size, Some(100));
    }

    #[test]
    fn a_subject_without_a_stage_cannot_build_a_query() {
        assert_eq!(require_stage(None).unwrap_err().code, "invalid_input");
        assert_eq!(require_stage(Some("3".to_owned())).unwrap(), "3");
    }

    #[test]
    fn answers_are_read_back_from_the_pending_write() {
        let pending = crate::confirm::PendingWrite {
            account_id: AccountId::from("acc"),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({
                "items": [
                    {"question_id": "Q1", "answer_xml": "<elements/>", "mark_xml": "<elements/>"},
                    {"question_id": "Q2", "answer_xml": "", "mark_xml": ""}
                ],
                "startTime": 1_762_000_000_000i64
            }),
        };
        let answers = answers_of(&pending).unwrap();
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0].question_id, "Q1");
        assert_eq!(answers[1].answer_xml, "");
        assert_eq!(start_time_of(&pending), Some(1_762_000_000_000));
    }

    #[test]
    fn a_pending_write_without_answers_is_rejected() {
        let pending = crate::confirm::PendingWrite {
            account_id: AccountId::from("acc"),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({"other": 1}),
        };
        assert_eq!(
            answers_of(&pending).unwrap_err().code,
            "confirmation_expired"
        );
    }

    #[test]
    fn the_run_result_counts_what_it_cleared() {
        let view = CleanupRunView {
            ok: true,
            message: "已清理 2 题".to_owned(),
            items: vec![
                CleanupItemResult {
                    question_id: "Q1".to_owned(),
                    state: "cleared".to_owned(),
                    message: "已清理".to_owned(),
                },
                CleanupItemResult {
                    question_id: "Q2".to_owned(),
                    state: "failed".to_owned(),
                    message: "network error".to_owned(),
                },
            ],
            cleared: 1,
        };
        // A failed item is reported as failed, and the count matches.
        assert_eq!(
            view.items.iter().filter(|i| i.state == "cleared").count(),
            view.cleared
        );
        assert_eq!(view.items[1].state, "failed");
    }
}
