//! Word PK commands: draw a round from the bucket, then submit it once.
//!
//! The user supplies `score` (correct words) and `total` (drawn words). The
//! client picks `total` distinct words from the task's `pkBucketId` bucket,
//! marks the first `score` correct and the rest wrong, and shows that exact
//! round before anything is sent.
//!
//! The two-step shape is required by the protocol: `-13` means "already
//! submitted / attempts exhausted", and a burned attempt cannot be un-burned.
//! So the preview carries the drawn round in its confirmation token: the submit
//! step can neither be replayed nor pointed at different words.

use serde::{Deserialize, Serialize};
use up366_core::{AccountId, RoundRequest};
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
    /// Whether this word is drawn as a correct answer.
    pub correct: bool,
}

/// What the confirmation dialog shows before a round is submitted.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WordPkPreviewView {
    pub job_id: String,
    pub title: String,
    pub bucket_id: String,
    /// Usable words the round was drawn from.
    pub pool_size: usize,
    /// Requested score, which is also the number of correct words.
    pub score: usize,
    /// Number of drawn words.
    pub total: usize,
    /// Drawn words in wire order: correct first, then wrong.
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

/// What a confirmation token carries: the round exactly as previewed.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PendingRound {
    job_id: String,
    course_id: String,
    bucket_id: String,
    score: usize,
    correct: Vec<Word>,
    wrong: Vec<Word>,
}

/// A word-PK task's bucket, resolved from the cached job row.
#[derive(Debug, Clone)]
struct Task {
    course_id: String,
    bucket_id: String,
}

/// Locate the word-PK task and its bucket from the cached job row.
///
/// The bucket id lives in `extParam.xotParam.pkBucketId`, which is a JSON
/// string nested inside another JSON string, so it is parsed defensively: a
/// missing or malformed value is an error the user can act on, not a panic.
fn task_for_job(job: &up366_store::JobRecord) -> CommandResult<Task> {
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

    Ok(Task {
        course_id: job.course_id.to_string(),
        bucket_id,
    })
}

/// A value that may itself hold JSON text.
fn as_json_or_str(value: Option<&serde_json::Value>) -> Option<serde_json::Value> {
    match value? {
        serde_json::Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

/// Draw a round: `score` correct words out of `total` randomly picked words.
///
/// Deliberately does not submit: the caller shows this and asks. The drawn
/// round is stored in the one-shot token, so the submit step cannot be replayed
/// and cannot submit a different round than the one previewed.
#[tauri::command(rename_all = "snake_case")]
pub async fn wordpk_preview(
    account_id: String,
    job_id: String,
    score: usize,
    total: usize,
    state: tauri::State<'_, AppState>,
) -> CommandResult<WordPkPreviewView> {
    let (active, client) = state.require_client(Some(&account_id))?;
    let job = match find_job(&state, &active, &job_id).await {
        Ok(job) => job,
        // A direct link or a cleared cache must not fail the picker: refresh the
        // catalog once, then look the job up again.
        Err(error) if error.code == "not_found" => {
            crate::commands::catalog::refresh_catalog(&state, &active, &client).await?;
            find_job(&state, &active, &job_id).await?
        }
        Err(error) => return Err(error),
    };
    let task = task_for_job(&job)?;

    let pool = usable_words(
        adapters::bucket_words(&client, &task.bucket_id)
            .await
            .map_err(|error| state.session_error(&active, &client, error))?,
    );
    if pool.is_empty() {
        return Err(AppError::new("api_error", "词库为空或单词缺少 entryId"));
    }

    let request = RoundRequest { score, total }.validate(pool.len())?;
    let picked = up366_core::draw_word_pk(request, pool.len(), seed());
    let drawn: Vec<Word> = picked.iter().map(|&index| pool[index].clone()).collect();
    let (correct, wrong) = drawn.split_at(request.score);

    let drawn_at = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let pending = PendingRound {
        job_id: job_id.clone(),
        course_id: task.course_id.clone(),
        bucket_id: task.bucket_id.clone(),
        score: request.score,
        correct: correct.to_vec(),
        wrong: wrong.to_vec(),
    };
    let payload = serde_json::to_value(&pending)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    let confirm_token = state.confirmations.issue(crate::confirm::PendingWrite {
        account_id: active,
        operation: "word_pk",
        payload: serde_json::json!({
            "jobId": job_id,
            "courseId": task.course_id,
            "bucketId": task.bucket_id,
            "score": request.score,
            "round": payload,
            "drawnAt": drawn_at,
        }),
    });

    Ok(WordPkPreviewView {
        job_id,
        title: job.name.clone(),
        bucket_id: task.bucket_id,
        pool_size: pool.len(),
        score: request.score,
        total: request.total,
        words: drawn
            .iter()
            .enumerate()
            .map(|(index, word)| word_view(word, index < request.score))
            .collect(),
        confirm_token,
    })
}

/// Submit the round the user confirmed. The token is consumed before the write.
#[tauri::command(rename_all = "snake_case")]
pub async fn wordpk_submit(
    confirm_token: String,
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

    let round: PendingRound = pending
        .payload
        .get("round")
        .cloned()
        .ok_or_else(|| AppError::new("confirmation_expired", "确认内容不完整"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| AppError::new("confirmation_expired", "确认内容不完整"))
        })?;
    if round.correct.is_empty() && round.wrong.is_empty() {
        return Err(AppError::new("confirmation_expired", "确认内容不完整"));
    }

    let now = adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let drawn_at = pending
        .payload
        .get("drawnAt")
        .and_then(|value| value.as_i64())
        .unwrap_or(now);
    let seconds = if drawn_at > 0 && now > drawn_at {
        (now - drawn_at) / 1000
    } else {
        0
    };

    let submission = WordPkSubmission {
        course_id: round.course_id,
        study_task_id: round.job_id,
        correct: round.correct.clone(),
        wrong: round.wrong.clone(),
        score: round.score as i64,
        duration: if seconds > 0 {
            seconds.to_string()
        } else {
            String::new()
        },
        role_name: "Red".to_owned(),
    };

    let data = adapters::submit_word_pk(&client, &submission)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    Ok(WordPkResult {
        ok: !data.is_null(),
        message: if data.is_null() {
            "服务器未返回结果".to_owned()
        } else {
            format!(
                "提交成功：{} 词，答对 {} 个",
                round.correct.len() + round.wrong.len(),
                round.correct.len()
            )
        },
    })
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

/// Render one drawn word.
fn word_view(word: &Word, correct: bool) -> WordView {
    WordView {
        word_id: word.id().unwrap_or_default().to_owned(),
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
        correct,
    }
}

/// Keep unique words the server can identify by id.
///
/// A bucket normally returns `entryId`; a few older buckets use `pkid`. A word
/// with neither cannot be submitted, so it is dropped rather than producing a
/// broken payload.
fn usable_words(words: Vec<Word>) -> Vec<Word> {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::with_capacity(words.len());
    for word in words {
        let Some(id) = word.id() else { continue };
        if seen.insert(id.to_owned()) {
            kept.push(word);
        }
    }
    kept
}

/// A fresh seed per draw: two previews of the same task must not pick the same
/// words, or the round the user confirmed could be replayed by a second open.
fn seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64).rotate_left(32)
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
    fn word_view_prefers_us_phonetic_and_audio() {
        let word: Word = serde_json::from_value(serde_json::json!({
            "entryId": "e1",
            "entry": "apple",
            "usPhonetic": "/ˈæpəl/",
            "enPhonetic": "/x/",
            "usVoiceFile": "a.mp3"
        }))
        .unwrap();
        let view = word_view(&word, true);
        assert_eq!(view.phonetic, "/ˈæpəl/");
        assert_eq!(view.audio_url.as_deref(), Some("a.mp3"));
        assert!(view.correct);
    }

    #[test]
    fn usable_words_drops_missing_ids_and_duplicates() {
        let mut all = words(2);
        all.push(serde_json::from_value(serde_json::json!({"entry": "no-id"})).unwrap());
        all.push(serde_json::from_value(serde_json::json!({"entryId": "e0"})).unwrap());
        let kept = usable_words(all);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].id(), Some("e0"));
    }

    #[test]
    fn usable_words_accepts_the_pkid_fallback() {
        let all = vec![
            serde_json::from_value(serde_json::json!({"pkid": "p1", "entry": "apple"})).unwrap(),
            serde_json::from_value(serde_json::json!({"entryId": "", "entry": "pear"})).unwrap(),
        ];
        let kept = usable_words(all);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id(), Some("p1"));
    }

    #[test]
    fn the_bucket_id_is_read_through_both_nested_json_strings() {
        let job = up366_store::JobRecord {
            course_id: up366_core::CourseId::from("9222783"),
            job_id: up366_core::JobId::from("JOB-1"),
            content_id: None,
            book_id: None,
            chapter_id: None,
            paper_id: None,
            name: "10.30 单词PK".to_owned(),
            order_index: 0,
            raw_json: serde_json::json!({
                "extParam": "{\"xotParam\": \"{\\\"pkBucketId\\\": \\\"BUCKET-1\\\"}\"}"
            })
            .to_string(),
            fetched_at: 0,
        };
        let task = task_for_job(&job).unwrap();
        assert_eq!(task.bucket_id, "BUCKET-1");
        assert_eq!(task.course_id, "9222783");
    }

    #[test]
    fn a_job_without_a_bucket_id_is_reported_not_panicked() {
        let job = up366_store::JobRecord {
            course_id: up366_core::CourseId::from("1"),
            job_id: up366_core::JobId::from("JOB-2"),
            content_id: None,
            book_id: None,
            chapter_id: None,
            paper_id: None,
            name: "普通作业".to_owned(),
            order_index: 0,
            raw_json: serde_json::json!({"extParam": "{}"}).to_string(),
            fetched_at: 0,
        };
        assert_eq!(task_for_job(&job).unwrap_err().code, "protocol_drift");
    }

    #[test]
    fn a_pending_round_round_trips_through_the_token_payload() {
        let pending = PendingRound {
            job_id: "JOB-1".to_owned(),
            course_id: "9222783".to_owned(),
            bucket_id: "BUCKET-1".to_owned(),
            score: 1,
            correct: words(1),
            wrong: words(1),
        };
        let value = serde_json::to_value(&pending).unwrap();
        let back: PendingRound = serde_json::from_value(value).unwrap();
        assert_eq!(back.score, 1);
        assert_eq!(back.job_id, "JOB-1");
        assert_eq!(back.correct.len() + back.wrong.len(), 2);
    }
}
