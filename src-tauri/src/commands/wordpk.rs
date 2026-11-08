//! Word PK commands: preview a round, then submit it once.
//!
//! The two-step shape is required by the protocol: `-13` means "already
//! submitted / attempts exhausted", and a burned attempt cannot be un-burned.
//! So the picker shows the exact round (which words, which score) and only then
//! submits, using a single-use token that the preview issued.

use serde::{Deserialize, Serialize};
use up366_core::AccountId;
use up366_protocol::{adapters, adapters::WordPkSubmission, dto::Word};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// One word as the picker renders it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordView {
    pub word_id: String,
    pub word: String,
    pub phonetic: String,
    pub translation: String,
    pub audio_url: Option<String>,
}

/// What the confirmation dialog shows before a round is submitted.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordPkPreviewView {
    pub job_id: String,
    pub title: String,
    pub bucket_id: String,
    pub words: Vec<WordView>,
    /// Token to pass back to `wordpk_submit`.
    pub confirm_token: String,
}

/// A completed round.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordPkResult {
    pub ok: bool,
    pub message: String,
}

/// The user's answer for one word.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WordPkAnswer {
    pub word_id: String,
    /// Whether the student got the word right.
    pub known: bool,
}

/// A pending round, re-fetched from the bucket instead of trusting the client.
#[derive(Debug, Clone)]
struct Round {
    course_id: String,
    bucket_id: String,
    words: Vec<Word>,
}

/// Locate the word-PK task and its bucket from the cached job row.
///
/// The bucket id lives in `extParam.xotParam.pkBucketId`, which is a JSON
/// string nested inside another JSON string, so it is parsed defensively: a
/// missing or malformed value is an error the user can act on, not a panic.
fn round_for_job(job: &up366_store::JobRecord) -> CommandResult<Round> {
    let raw: serde_json::Value = serde_json::from_str(&job.raw_json)
        .map_err(|error| AppError::new("protocol_drift", error.to_string()))?;

    // `as_json_or_str` either takes the value itself or parses its text, so
    // both nesting layers read the same way.
    let ext = as_json_or_str(raw.get("extParam"))
        .ok_or_else(|| AppError::new("protocol_drift", "作业缺少 extParam"))?;
    let bucket_id = as_json_or_str(ext.get("xotParam"))
        .as_ref()
        .and_then(|xot| xot.get("pkBucketId"))
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::new("protocol_drift", "作业缺少 pkBucketId"))?;

    Ok(Round {
        course_id: job.course_id.to_string(),
        bucket_id,
        words: Vec::new(),
    })
}

/// A value that may itself hold JSON text.
fn as_json_or_str(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    match value? {
        serde_json::Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

/// Load the bucket and build the picker view.
///
/// Deliberately does not submit: the caller shows this and asks. A one-shot
/// token is returned so the submit step cannot be replayed.
#[tauri::command(rename_all = "snake_case")]
pub async fn wordpk_preview(
    account_id: String,
    job_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<WordPkPreviewView> {
    let (active, client) = state.require_client(Some(&account_id))?;
    let job = find_job(&state, &active, &job_id).await?;
    let mut round = round_for_job(&job)?;

    round.words = adapters::bucket_words(&client, &round.bucket_id)
        .await
        .map_err(|error| state.session_error(&active, &client, error))?;
    if round.words.is_empty() {
        return Err(AppError::new("api_error", "词库为空"));
    }

    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: active,
        operation: "word_pk",
        payload: serde_json::json!({
            "jobId": job_id,
            "courseId": round.course_id,
            "bucketId": round.bucket_id,
        }),
    });

    Ok(WordPkPreviewView {
        job_id,
        title: job.name.clone(),
        bucket_id: round.bucket_id.clone(),
        words: round.words.iter().map(word_view).collect(),
        confirm_token,
    })
}

/// Submit the round the user confirmed.
///
/// The token is consumed first, so a double click or a stale retry fails
/// without touching the server. The bucket is re-read at submit time so the
/// submitted words are the words the server still returns.
#[tauri::command(rename_all = "snake_case")]
pub async fn wordpk_submit(
    confirm_token: String,
    answers: Vec<WordPkAnswer>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<WordPkResult> {
    let (account_id, client) = state.require_client(None)?;

    // Consume before any network call: an unusable token must not be able to
    // burn a submission attempt.
    let pending = state
        .confirmations
        .consume(&account_id, &confirm_token)
        .map_err(|failure| match failure {
            crate::confirm::Write::AccountMismatch => {
                AppError::new("account_mismatch", "确认令牌属于其他账号")
            }
            crate::confirm::Write::ConfirmationExpired => {
                AppError::new("confirmation_expired", "确认已过期或已使用，请重新打开预览")
            }
        })?;

    let bucket_id = pending
        .payload
        .get("bucketId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::new("confirmation_expired", "确认内容不完整"))?
        .to_owned();

    let words = adapters::bucket_words(&client, &bucket_id)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let (correct, wrong) = split_words(&words, &answers)?;

    // The score is the number of correct words; send exactly that.
    let submission = WordPkSubmission {
        course_id: course_id_of(&pending),
        study_task_id: job_id_of(&pending),
        correct: correct.clone(),
        wrong: wrong.clone(),
        score: correct.len() as i64,
        duration: String::new(),
    };

    let data = adapters::submit_word_pk(&client, &submission)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    Ok(WordPkResult {
        ok: !data.is_null(),
        message: if data.is_null() {
            "服务器未返回结果".to_owned()
        } else {
            "提交成功".to_owned()
        },
    })
}

fn course_id_of(pending: &crate::confirm::PendingWrite) -> String {
    pending
        .payload
        .get("courseId")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn job_id_of(pending: &crate::confirm::PendingWrite) -> String {
    pending
        .payload
        .get("jobId")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Find a cached job row by id across all courses.
async fn find_job(
    state: &AppState,
    account_id: &AccountId,
    job_id: &str,
) -> CommandResult<up366_store::JobRecord> {
    state
        .db
        .cache()
        .jobs(account_id, None)
        .await
        .map_err(|_| AppError::new("storage_error", "读取缓存失败"))?
        .into_iter()
        .find(|row| row.job_id.as_str() == job_id)
        .ok_or_else(|| AppError::new("not_found", "作业不在缓存中，请先刷新作业列表"))
}

/// Render one bucket word.
fn word_view(word: &Word) -> WordView {
    WordView {
        word_id: word.entry_id.clone().unwrap_or_default(),
        word: word.entry.clone().unwrap_or_default(),
        phonetic: word
            .us_phonetic
            .clone()
            .or_else(|| word.en_phonetic.clone())
            .unwrap_or_default(),
        translation: word.cn(),
        audio_url: word
            .us_voice_file
            .clone()
            .or_else(|| word.en_voice_file.clone()),
    }
}

/// Split answers into correct and wrong buckets.
///
/// Unknown and duplicate ids are both rejected: a round built from words the
/// server did not return would be a corrupt submission.
fn split_words(words: &[Word], answers: &[WordPkAnswer]) -> CommandResult<(Vec<Word>, Vec<Word>)> {
    let mut correct = Vec::new();
    let mut wrong = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for answer in answers {
        if !seen.insert(answer.word_id.as_str()) {
            return Err(AppError::new("invalid_input", "同一单词被重复作答"));
        }
        let word = words
            .iter()
            .find(|word| word.entry_id.as_deref() == Some(answer.word_id.as_str()))
            .cloned()
            .ok_or_else(|| {
                AppError::new("invalid_input", format!("未知单词: {}", answer.word_id))
            })?;
        if answer.known {
            correct.push(word);
        } else {
            wrong.push(word);
        }
    }
    if correct.is_empty() && wrong.is_empty() {
        return Err(AppError::new("invalid_input", "没有任何作答"));
    }
    Ok((correct, wrong))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(count: usize) -> Vec<Word> {
        (0..count)
            .map(|index| {
                serde_json::from_value(serde_json::json!({
                    "entryId": format!("e{index}"),
                    "entry": format!("word{index}")
                }))
                .unwrap()
            })
            .collect()
    }

    #[test]
    fn answers_are_split_by_known_flag() {
        let (correct, wrong) = split_words(
            &words(3),
            &[
                WordPkAnswer {
                    word_id: "e0".into(),
                    known: true,
                },
                WordPkAnswer {
                    word_id: "e1".into(),
                    known: false,
                },
                WordPkAnswer {
                    word_id: "e2".into(),
                    known: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(correct.len(), 2);
        assert_eq!(wrong.len(), 1);
    }

    #[test]
    fn an_answer_for_a_word_not_in_the_bucket_is_rejected() {
        let error = split_words(
            &words(1),
            &[WordPkAnswer {
                word_id: "missing".into(),
                known: true,
            }],
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn the_same_word_answered_twice_is_rejected() {
        let error = split_words(
            &words(1),
            &[
                WordPkAnswer {
                    word_id: "e0".into(),
                    known: true,
                },
                WordPkAnswer {
                    word_id: "e0".into(),
                    known: false,
                },
            ],
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn an_empty_round_is_rejected_before_any_request() {
        let error = split_words(&words(2), &[]).unwrap_err();
        assert_eq!(error.code, "invalid_input");
    }

    #[test]
    fn word_view_prefers_us_phonetic_and_audio() {
        let word: Word = serde_json::from_value(serde_json::json!({
            "entryId": "e1",
            "entry": "apple",
            "usPhonetic": "/ˈæpəl/",
            "enPhonetic": "/x/",
            "usVoiceFile": "a.mp3"
        }))
        .unwrap();
        let view = word_view(&word);
        assert_eq!(view.phonetic, "/ˈæpəl/");
        assert_eq!(view.audio_url.as_deref(), Some("a.mp3"));
    }

    #[test]
    fn the_bucket_id_is_read_through_both_nested_json_strings() {
        let job = up366_store::JobRecord {
            course_id: up366_core::CourseId::from("9222783"),
            job_id: up366_core::JobId::from("JOB-1"),
            content_id: None,
            book_id: None,
            chapter_id: None,
            name: "10.30 单词PK".to_owned(),
            order_index: 0,
            raw_json: serde_json::json!({
                "extParam": "{\"xotParam\": \"{\\\"pkBucketId\\\": \\\"BUCKET-1\\\"}\"}"
            })
            .to_string(),
            fetched_at: 0,
        };
        let round = round_for_job(&job).unwrap();
        assert_eq!(round.bucket_id, "BUCKET-1");
        assert_eq!(round.course_id, "9222783");
    }

    #[test]
    fn a_job_without_a_bucket_id_is_reported_not_panicked() {
        let job = up366_store::JobRecord {
            course_id: up366_core::CourseId::from("1"),
            job_id: up366_core::JobId::from("JOB-2"),
            content_id: None,
            book_id: None,
            chapter_id: None,
            name: "普通作业".to_owned(),
            order_index: 0,
            raw_json: serde_json::json!({"extParam": "{}"}).to_string(),
            fetched_at: 0,
        };
        assert_eq!(round_for_job(&job).unwrap_err().code, "protocol_drift");
    }
}
