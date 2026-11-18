//! Applied-writing (应用文) commands: detect the paper, preview the answer XML,
//! submit once.
//!
//! The paper is the authority: a writing question is `question_type = 14`
//! (`应用文写作`). A mixed paper is refused because this client can only answer
//! the writing part. The submit is the H5 player's branch
//! (`xot-api /client/onlinetesting/submit`), not the read-aloud score row.

use serde::{Deserialize, Serialize};
use up366_core::AccountId;
use up366_protocol::adapters::{
    self, build_answer_data, parse_applied_writing, submit_applied_writing, AppliedWritingPaper,
    AppliedWritingSubmission, RedoOutcome, WritingAnswer,
};

use crate::commands::{load_paper_context, retest_state, uid_for, verify_finished_job};
use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// One writing question as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedWritingQuestionView {
    pub question_id: String,
    pub number: String,
    pub score: f64,
    /// Prompt text with its paragraphs preserved.
    pub prompt: String,
    /// Suggested word count, when the paper declares one.
    pub word_num: Option<u32>,
    /// Reference duration in seconds.
    pub duration: u32,
}

/// A writing paper detected from the job's paper detail.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedWritingTaskView {
    pub job_id: String,
    pub title: String,
    pub submitted: bool,
    /// Whether the server will accept a retest for this submitted job.
    pub retest_supported: bool,
    /// Why a retest is unavailable; empty when it is available.
    pub retest_reason: String,
    /// Expired jobs are not blocked here; the UI only warns.
    pub expired: bool,
    pub total_score: f64,
    pub questions: Vec<AppliedWritingQuestionView>,
}

/// One essay supplied by the user.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WritingAnswerInput {
    pub question_id: String,
    pub text: String,
}

/// The confirmation dialog's payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedWritingPreviewView {
    pub job_id: String,
    pub title: String,
    pub total_score: f64,
    /// The submission will reset the previous attempt first.
    pub retest: bool,
    pub answers: Vec<PreviewAnswerView>,
    /// The exact `answerData` XML a submission would carry.
    pub answer_data: String,
    /// Single-use token for `applied_writing_submit`.
    pub confirm_token: String,
}

/// One answer in the confirmation dialog.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreviewAnswerView {
    pub question_id: String,
    pub number: String,
    pub text: String,
}

/// The result of one submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedWritingSubmitView {
    pub ok: bool,
    /// Whether the account-wide finished list already shows the job.
    pub verified: bool,
    /// The submission was a retest: the previous attempt was reset first.
    pub retested: bool,
    pub score: Option<f64>,
    pub message: String,
}

/// What a confirmation token carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingWriting {
    job_id: String,
    course_id: String,
    paper_id: String,
    /// The job's `contentId`, which is the `testId` the H5 page sends.
    content_id: String,
    uid: i64,
    answer_data: String,
    total_score: f64,
    seconds: i64,
    begin_time: i64,
    end_time: i64,
    /// Reset the previous attempt before posting this document.
    retest: bool,
}

struct LoadedWriting {
    job_id: String,
    course_id: String,
    title: String,
    paper_id: String,
    content_id: String,
    paper: AppliedWritingPaper,
    submitted: bool,
    retest_supported: bool,
    retest_reason: String,
    expired: bool,
}

/// Read the job's paper and return its writing questions.
#[tauri::command(rename_all = "snake_case")]
pub async fn applied_writing_get(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AppliedWritingTaskView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_writing(&state, &account_id, &client, &job_id).await?;
    Ok(task_view(&loaded))
}

/// Build the answer XML and issue a single-use confirmation token.
#[tauri::command(rename_all = "snake_case")]
pub async fn applied_writing_preview(
    account_id: String,
    job_id: String,
    answers: Vec<WritingAnswerInput>,
    duration_sec: i64,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AppliedWritingPreviewView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let loaded = load_writing(&state, &account_id, &client, &job_id).await?;
    let retest = if loaded.submitted {
        if !loaded.retest_supported {
            return Err(AppError::new(
                "duplicate_submission",
                if loaded.retest_reason.is_empty() {
                    "该作业已提交，不能重复提交".to_owned()
                } else {
                    format!("该作业已提交，不能重复提交：{}", loaded.retest_reason)
                },
            ));
        }
        true
    } else {
        false
    };

    // Reject an answer for a question that is not in the paper: it would be
    // silently dropped and make the count check misleading.
    for answer in &answers {
        if !loaded
            .paper
            .questions
            .iter()
            .any(|question| question.question_id == answer.question_id)
        {
            return Err(AppError::new(
                "invalid_input",
                format!("题目 {} 不属于这份试卷", answer.question_id),
            ));
        }
    }
    let answers: Vec<WritingAnswer> = answers
        .into_iter()
        .map(|answer| WritingAnswer {
            question_id: answer.question_id,
            text: answer.text,
        })
        .collect();
    let answer_data = build_answer_data(&loaded.paper, &answers).map_err(AppError::from)?;

    let uid: i64 = uid_for(&state, &account_id, &client)
        .await?
        .parse()
        .map_err(|_| AppError::new("protocol_drift", "uid 不是数字"))?;
    let end_time = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let seconds = duration_sec.clamp(1, 86_400);
    let pending = PendingWriting {
        job_id: loaded.job_id.clone(),
        course_id: loaded.course_id.clone(),
        paper_id: loaded.paper_id.clone(),
        content_id: loaded.content_id.clone(),
        uid,
        answer_data: answer_data.clone(),
        total_score: loaded.paper.total_score,
        seconds,
        begin_time: end_time.saturating_sub(seconds.saturating_mul(1000)),
        end_time,
        retest,
    };
    let payload = serde_json::to_value(&pending)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: account_id.clone(),
        operation: "applied_writing_submit",
        payload,
    });

    let preview_answers = loaded
        .paper
        .questions
        .iter()
        .filter_map(|question| {
            answers
                .iter()
                .find(|answer| answer.question_id == question.question_id)
                .map(|answer| PreviewAnswerView {
                    question_id: question.question_id.clone(),
                    number: question.number.clone(),
                    text: answer.text.trim().to_owned(),
                })
        })
        .collect();

    Ok(AppliedWritingPreviewView {
        job_id: loaded.job_id,
        title: loaded.title,
        total_score: loaded.paper.total_score,
        retest,
        answers: preview_answers,
        answer_data,
        confirm_token,
    })
}

/// Submit the previewed answer document. The token is single-use.
#[tauri::command(rename_all = "snake_case")]
pub async fn applied_writing_submit(
    confirm_token: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AppliedWritingSubmitView> {
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
    let pending: PendingWriting = serde_json::from_value(pending.payload)
        .map_err(|error| AppError::new("protocol_drift", error.to_string()))?;
    if pending.answer_data.is_empty() {
        return Err(AppError::new("confirmation_expired", "确认内容不完整"));
    }

    if pending.retest {
        match adapters::redo_homework(&client, &pending.content_id)
            .await
            .map_err(|error| state.session_error(&account_id, &client, error))?
        {
            RedoOutcome::Accepted => {}
            RedoOutcome::Closed { message } => {
                return Err(AppError::new(
                    "retest_closed",
                    if message.is_empty() {
                        "练习已经结束，不可以再重新测试".to_owned()
                    } else {
                        message
                    },
                ))
            }
            RedoOutcome::Unsupported { message } => {
                return Err(AppError::new(
                    "retest_unsupported",
                    if message.is_empty() {
                        "该练习不支持重做".to_owned()
                    } else {
                        message
                    },
                ))
            }
        }
    }

    let submission = AppliedWritingSubmission {
        test_id: pending.content_id.clone(),
        paper_id: pending.paper_id.clone(),
        uid: pending.uid,
        answer_data: pending.answer_data.clone(),
        duration: pending.seconds,
        begin_time: pending.begin_time,
        end_time: pending.end_time,
    };
    submit_applied_writing(&client, &submission)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    // A retest reuses the finished row: its score still describes the previous
    // attempt, so reading it back would report the wrong result.
    if pending.retest {
        return Ok(AppliedWritingSubmitView {
            ok: true,
            verified: false,
            retested: true,
            score: None,
            message: "已重置上一次作答并重新提交，分数以服务端批改结果为准".to_owned(),
        });
    }

    let (verified, score, _batch_id) = verify_finished_job(
        &state,
        &account_id,
        &client,
        &pending.job_id,
        &pending.course_id,
        pending.total_score,
    )
    .await?;

    Ok(AppliedWritingSubmitView {
        ok: true,
        verified,
        retested: false,
        score,
        message: if verified {
            "提交成功".to_owned()
        } else {
            "服务端已返回成功，但完成列表尚未刷新，请稍后重新加载作业".to_owned()
        },
    })
}

/// Load the job, its paper and the already-submitted state.
async fn load_writing(
    state: &AppState,
    account_id: &AccountId,
    client: &up366_protocol::Client,
    job_id: &str,
) -> CommandResult<LoadedWriting> {
    let context = load_paper_context(state, account_id, client, job_id).await?;
    let paper = parse_applied_writing(&context.paper_xml).map_err(AppError::from)?;
    paper.require_all_writing().map_err(AppError::from)?;
    let (retest_supported, retest_reason) = retest_state(
        state,
        account_id,
        client,
        &context.content_id,
        context.submitted,
    )
    .await?;

    Ok(LoadedWriting {
        job_id: context.job_id,
        course_id: context.course_id,
        title: context.title,
        paper_id: context.paper_id,
        content_id: context.content_id,
        paper,
        submitted: context.submitted,
        retest_supported,
        retest_reason,
        expired: context.expired,
    })
}

fn task_view(loaded: &LoadedWriting) -> AppliedWritingTaskView {
    AppliedWritingTaskView {
        job_id: loaded.job_id.clone(),
        title: loaded.title.clone(),
        submitted: loaded.submitted,
        retest_supported: loaded.retest_supported,
        retest_reason: loaded.retest_reason.clone(),
        expired: loaded.expired,
        total_score: loaded.paper.total_score,
        questions: loaded
            .paper
            .questions
            .iter()
            .map(|question| AppliedWritingQuestionView {
                question_id: question.question_id.clone(),
                number: question.number.clone(),
                score: question.score,
                prompt: question.prompt.clone(),
                word_num: question.word_num,
                duration: question.duration,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use up366_protocol::adapters::{AppliedWritingPaper, WritingQuestion};

    fn loaded() -> LoadedWriting {
        LoadedWriting {
            job_id: "JOB-1".to_owned(),
            course_id: "COURSE-1".to_owned(),
            title: "应用文练习".to_owned(),
            paper_id: "PAPER-1".to_owned(),
            content_id: "CONTENT-1".to_owned(),
            paper: AppliedWritingPaper {
                questions: vec![WritingQuestion {
                    question_id: "Q1".to_owned(),
                    number: "1".to_owned(),
                    score: 15.0,
                    duration: 1200,
                    prompt: "Write a letter.".to_owned(),
                    word_num: Some(80),
                }],
                other_questions: 0,
                total_score: 15.0,
            },
            submitted: false,
            retest_supported: false,
            retest_reason: String::new(),
            expired: false,
        }
    }

    #[test]
    fn the_task_view_keeps_the_detected_writing_question() {
        let view = task_view(&loaded());
        assert_eq!(view.job_id, "JOB-1");
        assert_eq!(view.total_score, 15.0);
        assert_eq!(view.questions.len(), 1);
        assert_eq!(view.questions[0].number, "1");
        assert_eq!(view.questions[0].word_num, Some(80));
    }

    #[test]
    fn a_pending_writing_round_trips_through_the_token_payload() {
        let pending = PendingWriting {
            job_id: "JOB-1".to_owned(),
            course_id: "COURSE-1".to_owned(),
            paper_id: "PAPER-1".to_owned(),
            content_id: "CONTENT-1".to_owned(),
            uid: 39370562,
            answer_data: "<elements/>".to_owned(),
            total_score: 15.0,
            seconds: 42,
            begin_time: 1_700_000_000_000,
            end_time: 1_700_000_042_000,
            retest: false,
        };
        let value: Value = serde_json::to_value(&pending).unwrap();
        let back: PendingWriting = serde_json::from_value(value).unwrap();
        assert_eq!(back.job_id, "JOB-1");
        assert_eq!(back.seconds, 42);
        assert_eq!(back.answer_data, "<elements/>");
    }
}
