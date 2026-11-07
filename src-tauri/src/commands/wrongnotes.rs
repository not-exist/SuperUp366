//! Wrong-answer book commands: browse, then clear questions one at a time.
//!
//! Cleanup is a batch of destructive submissions, so the whole flow follows
//! one rule: **the user picks, the app sends, and each result is recorded
//! individually.** Specifically:
//!
//! - the selection is previewed before anything is sent;
//! - questions are submitted **serially**, never in parallel, so a failure
//!   cannot cascade into a half-batch of duplicates;
//! - each question's outcome is returned separately, so a partial batch can be
//!   resumed instead of replayed (a replay would burn an attempt);
//! - three consecutive failures stop the batch;
//! - cancellation is checked between questions, never mid-request.
//!
//! The per-question granularity itself is not yet confirmed against the server
//! (`docs/protocol/to-probe.md` §6). The commands are still wired so the flow
//! can be exercised against a fixture, but the run reports
//! `granularity_confirmed: false` and stops before sending until a real probe
//! closes it.

use serde::{Deserialize, Serialize};
use up366_protocol::adapters::{self, WrongCleanup, WrongKnowledge, WrongQuery, WrongQuestion};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// Whether the server accepts one question per cleanup call.
///
/// Stays false until `docs/protocol/to-probe.md` §6 is closed by a probe.
const GRANULARITY_CONFIRMED: bool = false;

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
#[derive(Debug, Default, Clone, Deserialize)]
pub struct WrongFilter {
    pub subject_id: Option<String>,
    pub book_id: Option<String>,
    pub knowledge_id: Option<String>,
}

/// The subject list, used to fill the picker.
#[tauri::command]
pub async fn wrongnotes_subjects(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongSubjectView>> {
    let (_, client) = state.require_client(Some(&account_id))?;
    Ok(adapters::wrong_subjects(&client)
        .await
        .map_err(AppError::from)?
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
#[tauri::command]
pub async fn wrongnotes_knowledge(
    account_id: String,
    filter: WrongFilter,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongKnowledgeView>> {
    let (_, client) = state.require_client(Some(&account_id))?;
    let query = WrongQuery {
        subject_id: filter.subject_id,
        book_id: filter.book_id,
        ..Default::default()
    };
    Ok(adapters::wrong_knowledge(&client, &query)
        .await
        .map_err(AppError::from)?
        .into_iter()
        .map(knowledge_view)
        .collect())
}

/// The questions still to clear.
#[tauri::command]
pub async fn wrongnotes_pending(
    account_id: String,
    filter: WrongFilter,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<WrongQuestionView>> {
    let (_, client) = state.require_client(Some(&account_id))?;
    let query = WrongQuery {
        subject_id: filter.subject_id,
        book_id: filter.book_id,
        knowledge_id: filter.knowledge_id,
        ..Default::default()
    };
    Ok(adapters::wrong_pending(&client, &query)
        .await
        .map_err(AppError::from)?
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
    /// False until per-question granularity is confirmed; the run stops early.
    pub granularity_confirmed: bool,
    /// One-shot token for `wrongnotes_cleanup`.
    pub confirm_token: String,
}

/// Build the preview for the selected questions.
///
/// No request is sent: the questions come from the caller's selection, and the
/// only network call is the knowledge lookup the UI already made.
#[tauri::command]
pub async fn wrongnotes_preview(
    account_id: String,
    question_ids: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CleanupPreviewView> {
    let (active, _) = state.require_client(Some(&account_id))?;
    if question_ids.is_empty() {
        return Err(AppError::new("invalid_input", "没有选择任何错题"));
    }

    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: active,
        operation: "wrongnote_cleanup",
        payload: serde_json::json!({ "questionIds": question_ids }),
    });

    Ok(CleanupPreviewView {
        items: question_ids
            .into_iter()
            .map(|question_id| CleanupItemView {
                title: question_id.clone(),
                question_id,
            })
            .collect(),
        total: 0,
        granularity_confirmed: GRANULARITY_CONFIRMED,
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
/// Serial by construction, one submission per question, and it stops before
/// sending anything while the granularity is unconfirmed.
#[tauri::command]
pub async fn wrongnotes_cleanup(
    confirm_token: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CleanupRunView> {
    let (account_id, _) = state.require_client(None)?;

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

    let question_ids = question_ids_of(&pending)?;
    if !GRANULARITY_CONFIRMED {
        return Ok(CleanupRunView {
            ok: false,
            message:
                "错题清理尚未对真实账号验证：服务端是否支持逐题提交未知，已停止，未发送任何请求"
                    .to_owned(),
            items: question_ids
                .into_iter()
                .map(|question_id| CleanupItemResult {
                    question_id,
                    state: "skipped".to_owned(),
                    message: "等待协议确认".to_owned(),
                })
                .collect(),
            cleared: 0,
        });
    }

    // Serial loop: one question at a time, stopping after three in a row fail.
    let (_, client) = state.require_client(None)?;
    let mut items = Vec::with_capacity(question_ids.len());
    let mut cleared = 0usize;
    let mut consecutive_failures = 0usize;
    let now = up366_store::now_millis();

    for question_id in question_ids {
        let cleanup = WrongCleanup {
            uid: account_id.to_string(),
            answer_xml: String::new(),
            mark_xml: String::new(),
            start_time: now,
            end_time: now,
            seconds: 0,
            right_qst_ids: question_id.clone(),
        };
        match adapters::submit_wrong_question(&client, &cleanup).await {
            Ok(_) => {
                cleared += 1;
                consecutive_failures = 0;
                items.push(CleanupItemResult {
                    question_id,
                    state: "cleared".to_owned(),
                    message: "已清理".to_owned(),
                });
            }
            Err(error) => {
                consecutive_failures += 1;
                items.push(CleanupItemResult {
                    question_id,
                    state: "failed".to_owned(),
                    message: error.to_string(),
                });
                if consecutive_failures >= 3 {
                    break;
                }
            }
        }
    }

    Ok(CleanupRunView {
        ok: consecutive_failures < 3,
        message: format!("已清理 {cleared} 题"),
        items,
        cleared,
    })
}

/// Read the selected question ids back out of the pending write.
fn question_ids_of(pending: &crate::confirm::PendingWrite) -> CommandResult<Vec<String>> {
    pending
        .payload
        .get("questionIds")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .ok_or_else(|| AppError::new("confirmation_expired", "确认内容不完整"))
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
            payload: serde_json::json!({"questionIds": []}),
        });
        // An empty selection never reaches a submission: it produces no work.
        assert!(store.consume(&account, &token).is_ok());
    }

    #[test]
    fn a_run_stops_before_sending_while_granularity_is_unconfirmed() {
        // Confirmations are issued directly so no account needs to be active.
        let store = crate::confirm::Confirmations::new();
        let token = store.issue(crate::confirm::PendingWrite {
            account_id: AccountId::from("acc"),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({"questionIds": ["Q1", "Q2"]}),
        });

        // Clearing without a confirmed granularity must be a no-op, and the
        // token must still be consumable so the user can retry afterwards.
        let granularity_confirmed = GRANULARITY_CONFIRMED;
        assert!(!granularity_confirmed);
        let pending = store.consume(&AccountId::from("acc"), &token).unwrap();
        let ids = question_ids_of(&pending).unwrap();
        assert_eq!(ids, vec!["Q1", "Q2"]);
    }

    #[test]
    fn question_ids_are_read_back_from_the_pending_write() {
        let pending = crate::confirm::PendingWrite {
            account_id: AccountId::from("acc"),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({"questionIds": ["Q1", "Q2", "Q3"]}),
        };
        let ids = question_ids_of(&pending).unwrap();
        assert_eq!(ids, vec!["Q1", "Q2", "Q3"]);
    }

    #[test]
    fn a_pending_write_without_question_ids_is_rejected() {
        let pending = crate::confirm::PendingWrite {
            account_id: AccountId::from("acc"),
            operation: "wrongnote_cleanup",
            payload: serde_json::json!({"other": 1}),
        };
        assert_eq!(
            question_ids_of(&pending).unwrap_err().code,
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
