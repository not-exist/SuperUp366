//! Error-notebook correction ("订正"): draw a paper, show it, submit it once.
//!
//! The draw response is the entire preview: the server returns the paper, the
//! standard answers and the batch id in one call. The command stores that paper
//! in a single-use confirmation token, so `correction_submit` cannot be replayed
//! and cannot be pointed at a different paper.

use serde::{Deserialize, Serialize};
use up366_protocol::adapters::{
    self, build_tasks_json, clear_wrong_notes, correction_draw as draw_correction,
    correction_overview as read_overview, correction_status as read_status, submit_correction,
    CorrectionContext, CorrectionPaper, CorrectionQuestion,
};

use crate::commands::uid_for;
use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// One question shown after drawing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionQuestionView {
    pub question_id: String,
    pub number: String,
    pub question_type: String,
    pub type_name: String,
    pub score: f64,
    pub stem: String,
    /// The correct answer the server sent with the paper.
    pub answer: String,
}

/// A drawn paper plus the one-shot token for submitting it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionDrawView {
    pub batch_id: String,
    pub path_prefix: String,
    pub questions: Vec<CorrectionQuestionView>,
    pub total_score: f64,
    /// `None` when there is nothing left to correct.
    pub confirm_token: Option<String>,
}

/// Overview counts and groups.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionOverviewView {
    pub remaining_count: i64,
    pub corrected_count: i64,
    pub today_corrected_count: i64,
    pub groups: Vec<CorrectionGroupView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionGroupView {
    pub group_key: String,
    pub remaining_count: i64,
    pub qtype_name: String,
    pub knowledge_name: String,
}

/// Marking state for one batch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionStatusView {
    pub status: i64,
    pub percent: f64,
    pub submit_date: i64,
}

/// The result of one submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CorrectionSubmitView {
    pub ok: bool,
    pub message: String,
    pub batch_id: String,
    /// Outcome of the "错题消灭" follow-up, when one was attempted.
    pub note_message: Option<String>,
}

/// What a confirmation token carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingCorrection {
    paper: CorrectionPaper,
    context: CorrectionContext,
    drawn_at: i64,
}

/// Counts and question-type groups.
#[tauri::command(rename_all = "snake_case")]
pub async fn correction_overview(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CorrectionOverviewView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let user = adapters::current_user(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let organ = number_or_default(user.school_id.as_deref());
    let stage = number_or_default(user.school_type.as_deref());
    let uid = number_or_default(Some(&user.uid));
    if uid == 0 {
        return Err(AppError::new("protocol_drift", "当前账号缺少 uid"));
    }
    let overview = read_overview(&client, uid, organ, stage)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    Ok(CorrectionOverviewView {
        remaining_count: overview.remaining_count,
        corrected_count: overview.corrected_count,
        today_corrected_count: overview.today_corrected_count,
        groups: overview
            .group_list
            .into_iter()
            .map(|group| CorrectionGroupView {
                group_key: group.group_key,
                remaining_count: group.remaining_count,
                qtype_name: group.question_type_name,
                knowledge_name: group.knowledge_point_name,
            })
            .collect(),
    })
}

/// Draw a correction paper.
///
/// `count` is the number of questions; `group_key` narrows it to one overview
/// group. The returned paper is only a preview until `correction_submit`.
#[tauri::command(rename_all = "snake_case")]
pub async fn correction_draw(
    account_id: String,
    count: i64,
    group_key: Option<String>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CorrectionDrawView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    if !(1..=50).contains(&count) {
        return Err(AppError::new("invalid_input", "订正题量需在 1 到 50 之间"));
    }
    let uid = uid_for(&state, &account_id, &client).await?;
    let student_id: i64 = uid
        .parse()
        .map_err(|_| AppError::new("protocol_drift", "uid 不是数字"))?;

    let drawn = draw_correction(&client, student_id, &[], count, group_key.as_deref())
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let Some(paper) = drawn else {
        return Ok(CorrectionDrawView {
            batch_id: String::new(),
            path_prefix: String::new(),
            questions: Vec::new(),
            total_score: 0.0,
            confirm_token: None,
        });
    };

    let drawn_at = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let context = CorrectionContext {
        uid,
        client_id: client.client_id().to_owned(),
        course_id: -1,
        study_task_id: String::new(),
    };
    let pending = PendingCorrection {
        paper: paper.clone(),
        context,
        drawn_at,
    };
    let payload = serde_json::to_value(&pending)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id,
        operation: "correction_submit",
        payload,
    });

    Ok(CorrectionDrawView {
        batch_id: paper.batch_id.clone(),
        path_prefix: paper.path_prefix.clone(),
        questions: paper
            .questions
            .iter()
            .map(|question| question_view(&paper, question))
            .collect(),
        total_score: paper.total_score(),
        confirm_token: Some(confirm_token),
    })
}

/// Submit the drawn paper. The token is single-use.
#[tauri::command(rename_all = "snake_case")]
pub async fn correction_submit(
    confirm_token: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CorrectionSubmitView> {
    let (account_id, client) = state.require_client(None)?;
    let pending = state
        .confirmations
        .consume(&account_id, &confirm_token)
        .map_err(|failure| match failure {
            crate::confirm::Write::AccountMismatch => {
                AppError::new("account_mismatch", "确认令牌属于其他账号")
            }
            crate::confirm::Write::ConfirmationExpired => {
                AppError::new("confirmation_expired", "确认已过期或已使用，请重新抽题")
            }
        })?;

    let pending: PendingCorrection = serde_json::from_value(pending.payload)
        .map_err(|_| AppError::new("confirmation_expired", "确认内容不完整"))?;
    if pending.paper.questions.is_empty() {
        return Err(AppError::new("confirmation_expired", "确认内容不完整"));
    }

    let now = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let seconds = if pending.drawn_at > 0 && now > pending.drawn_at {
        ((now - pending.drawn_at) / 1000).max(1)
    } else {
        1
    };
    let tasks = build_tasks_json(&pending.paper, &pending.context, seconds, now, "PRODUCT");
    submit_correction(&client, &tasks, client.now_ms())
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    // "错题消灭" is a separate write for job-anchored corrections. A failure
    // here must not report the submission as failed.
    let note_message = if pending.context.study_task_id.trim().is_empty() {
        None
    } else {
        match clear_wrong_notes(
            &client,
            &pending.context.uid,
            pending.context.course_id,
            &pending.context.study_task_id,
        )
        .await
        {
            Ok(_) => Some("错题已关联到作业".to_owned()),
            Err(error) => Some(format!("错题关联失败：{error}")),
        }
    };

    Ok(CorrectionSubmitView {
        ok: true,
        message: format!(
            "已提交 {} 道题，订单号 {}",
            pending.paper.questions.len(),
            pending.paper.batch_id
        ),
        batch_id: pending.paper.batch_id,
        note_message,
    })
}

/// Marking status for one batch.
#[tauri::command(rename_all = "snake_case")]
pub async fn correction_status(
    account_id: String,
    batch_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<CorrectionStatusView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let uid = uid_for(&state, &account_id, &client).await?;
    let status = read_status(&client, &batch_id, &uid)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    Ok(CorrectionStatusView {
        status: status.status,
        percent: status.percent,
        submit_date: status.submit_date,
    })
}

fn question_view(paper: &CorrectionPaper, question: &CorrectionQuestion) -> CorrectionQuestionView {
    CorrectionQuestionView {
        question_id: question.question_id.clone(),
        number: question.number.clone(),
        question_type: question.question_type.clone(),
        type_name: question.type_name.clone(),
        score: question.score,
        stem: question.stem.clone(),
        answer: paper.answer_text(&question.question_id),
    }
}

fn number_or_default(value: Option<&str>) -> i64 {
    value
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn a_pending_correction_round_trips_through_the_token_payload() {
        let pending = PendingCorrection {
            paper: CorrectionPaper {
                batch_id: "B1".to_owned(),
                path_prefix: "https://cdn.test".to_owned(),
                paper_xml: "<elements/>".to_owned(),
                answer_xml: "<elements/>".to_owned(),
                questions: Vec::new(),
            },
            context: CorrectionContext {
                uid: "42".to_owned(),
                client_id: "CID".to_owned(),
                course_id: -1,
                study_task_id: String::new(),
            },
            drawn_at: 1_700_000_000_000,
        };
        let value: Value = serde_json::to_value(&pending).unwrap();
        let back: PendingCorrection = serde_json::from_value(value).unwrap();
        assert_eq!(back.paper.batch_id, "B1");
        assert_eq!(back.context.uid, "42");
        assert_eq!(back.drawn_at, 1_700_000_000_000);
    }

    #[test]
    fn a_non_numeric_stage_or_organ_becomes_zero() {
        assert_eq!(number_or_default(Some("3105")), 3105);
        assert_eq!(number_or_default(Some(" 3 ")), 3);
        assert_eq!(number_or_default(Some("")), 0);
        assert_eq!(number_or_default(None), 0);
    }
}
