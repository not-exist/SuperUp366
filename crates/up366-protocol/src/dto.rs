//! Strongly typed DTOs for the read-only adapters.
//!
//! Upstream is loose about types: the same id may arrive as `9222783` or
//! `"9222783"`, booleans may be `true`, `1` or `"1"`, and structured fields
//! are sometimes JSON strings. Every field therefore uses a tolerant
//! deserializer from [`flex`], and every struct keeps the original object in
//! `raw` so callers can still reach fields this client does not model.
//!
//! These are response models only. Requests take plain `serde_json::Value`
//! fields, which keeps call sites readable.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::error::{ProtocolError, Result};

/// DTOs that retain their original JSON object.
pub trait RawPayload {
    /// Replace the stored raw payload.
    fn set_raw(&mut self, raw: Value);
}

/// Deserialize one item and preserve its original JSON in `raw`.
pub(crate) fn decode<T>(value: Value, endpoint: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned + RawPayload,
{
    let raw = value.clone();
    let mut decoded: T = serde_json::from_value(value)
        .map_err(|error| ProtocolError::Decode(format!("{endpoint}: {error}")))?;
    decoded.set_raw(raw);
    Ok(decoded)
}

macro_rules! impl_raw {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RawPayload for $ty {
                fn set_raw(&mut self, raw: Value) {
                    self.raw = raw;
                }
            }
        )*
    };
}

/// Tolerant field deserializers.
pub mod flex {
    use super::*;

    fn value<'de, D>(deserializer: D) -> std::result::Result<Option<Value>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<Value>::deserialize(deserializer)
    }

    pub fn opt_string<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match value(deserializer)? {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) => Some(text),
            Some(Value::Number(number)) => Some(number.to_string()),
            Some(Value::Bool(boolean)) => Some(boolean.to_string()),
            Some(other) => Some(other.to_string()),
        })
    }

    pub fn string_or_empty<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(opt_string(deserializer)?.unwrap_or_default())
    }

    pub fn opt_i64<'de, D>(deserializer: D) -> std::result::Result<Option<i64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match value(deserializer)? {
            Some(Value::Number(number)) => number
                .as_i64()
                .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
                .or_else(|| number.as_f64().map(|value| value as i64)),
            Some(Value::String(text)) => text.trim().parse().ok(),
            _ => None,
        })
    }

    pub fn opt_f64<'de, D>(deserializer: D) -> std::result::Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match value(deserializer)? {
            Some(Value::Number(number)) => number.as_f64(),
            Some(Value::String(text)) => text.trim().parse().ok(),
            _ => None,
        })
    }

    pub fn opt_bool<'de, D>(deserializer: D) -> std::result::Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match value(deserializer)? {
            Some(Value::Bool(boolean)) => Some(boolean),
            Some(Value::Number(number)) => number.as_i64().map(|value| value != 0),
            Some(Value::String(text)) => match text.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => Some(true),
                "0" | "false" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        })
    }

    /// Accept either a JSON object or a JSON-encoded string.
    pub fn ext_param<'de, D>(deserializer: D) -> std::result::Result<Option<Value>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match value(deserializer)? {
            Some(Value::String(text)) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(serde_json::from_str(trimmed).unwrap_or(Value::String(text)))
                }
            }
            other => other,
        })
    }
}

/// One course shown on the student dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub stage_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

impl Course {
    pub fn id(&self) -> Option<&str> {
        self.course_id.as_deref()
    }

    pub fn name(&self) -> &str {
        self.course_name.as_deref().unwrap_or("")
    }
}

/// A homework / word-picking task from `data.allTaskList`.
///
/// `extParam` is the authoritative source for most ids and states in the list
/// response, so [`Job::id`]-style accessors use the normalised fields filled by
/// [`Job::from_value`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub job_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub study_task_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub job_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub content_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub chapter_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub paper_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub task_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub begin_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub end_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub submit_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub task_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub stu_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub task_score: Option<f64>,
    #[serde(default, rename = "type", deserialize_with = "flex::opt_i64")]
    pub job_type: Option<i64>,
    #[serde(default, deserialize_with = "flex::ext_param")]
    pub ext_param: Option<Value>,
    #[serde(default)]
    pub raw: Value,
}

impl Job {
    /// Decode a job and fold `extParam` into the top-level fields.
    pub fn from_value(value: Value) -> Result<Self> {
        let mut job: Job = decode(value, "jobs")?;
        job.normalize();
        Ok(job)
    }

    fn normalize(&mut self) {
        // `xotParam` is sometimes a JSON string inside `extParam`.
        if let Some(Value::Object(ext)) = &mut self.ext_param {
            if let Some(Value::String(text)) = ext.get("xotParam") {
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    ext.insert("xotParam".to_owned(), parsed);
                }
            }
        }

        if self.course_id.is_none() {
            self.course_id = ext_string(&self.ext_param, "courseId");
        }
        if self.book_id.is_none() {
            self.book_id = ext_string(&self.ext_param, "bookId");
        }
        if self.chapter_id.is_none() {
            self.chapter_id = ext_string(&self.ext_param, "chapterId");
        }
        if self.paper_id.is_none() {
            self.paper_id = ext_string(&self.ext_param, "paperId");
        }
        if self.task_id.is_none() {
            self.task_id =
                ext_string(&self.ext_param, "taskId").or_else(|| self.content_id.clone());
        }
        if self.task_status.is_none() {
            self.task_status = ext_i64(&self.ext_param, "taskStatus");
        }
        if self.stu_status.is_none() {
            self.stu_status = ext_i64(&self.ext_param, "stuStatus");
        }
        if self.submit_time.is_none() {
            self.submit_time = ext_string(&self.ext_param, "submitTime");
        }
        if self.task_score.is_none() {
            self.task_score = ext_f64(&self.ext_param, "taskScore");
        }
    }

    pub fn id(&self) -> Option<&str> {
        self.job_id.as_deref().or(self.study_task_id.as_deref())
    }

    pub fn title(&self) -> &str {
        self.job_name
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("")
    }

    pub fn course(&self) -> Option<&str> {
        self.course_id.as_deref()
    }

    pub fn content(&self) -> Option<&str> {
        self.content_id.as_deref()
    }

    pub fn book(&self) -> Option<&str> {
        self.book_id.as_deref()
    }

    pub fn paper(&self) -> Option<&str> {
        self.paper_id.as_deref()
    }

    /// `taskId` used by `answer_detail`; equals `content_id`, never `jobId`.
    pub fn task(&self) -> Option<&str> {
        self.task_id.as_deref()
    }

    pub fn status(&self) -> Option<i64> {
        self.task_status
    }

    pub fn student_status(&self) -> Option<i64> {
        self.stu_status
    }

    pub fn finished(&self) -> bool {
        self.task_status == Some(1) || self.stu_status.is_some_and(|status| status > 1)
    }

    fn xot_param(&self) -> Option<&Map<String, Value>> {
        self.ext_param.as_ref()?.get("xotParam")?.as_object()
    }

    /// Whether this is a word-picking (WordPkx) card.
    pub fn is_word_pk(&self) -> bool {
        let Some(xot) = self.xot_param() else {
            return false;
        };
        let has_bucket = xot
            .get("pkBucketId")
            .is_some_and(|value| !value.is_null() && value.as_str() != Some(""));
        has_bucket || xot.contains_key("wordpkPlayType")
    }

    pub fn kind(&self) -> up366_core::JobKind {
        if self.is_word_pk() {
            up366_core::JobKind::WordPk
        } else {
            up366_core::JobKind::hint_from_title(self.title())
        }
    }

    /// Cheap hint that a job contains read-aloud questions.
    ///
    /// The authoritative answer needs the paper (`question_type = 9`); this
    /// only decides whether the UI offers the read-aloud page. Upstream names
    /// such duties ".../逐句跟读" or ".../朗读".
    pub fn is_read_aloud_hint(&self) -> bool {
        let title = self.title();
        if title.contains("跟读") || title.contains("朗读") {
            return true;
        }
        self.ext_param
            .as_ref()
            .and_then(|ext| ext.get("openType").or_else(|| ext.get("open_type")))
            .and_then(Value::as_str)
            .is_some_and(|open| open.to_ascii_lowercase().contains("listenandread"))
    }
}

fn ext_string(ext_param: &Option<Value>, key: &str) -> Option<String> {
    let value = ext_param.as_ref()?.get(key)?;
    crate::codec::value_to_string(value)
}

fn ext_i64(ext_param: &Option<Value>, key: &str) -> Option<i64> {
    let value = ext_param.as_ref()?.get(key)?;
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn ext_f64(ext_param: &Option<Value>, key: &str) -> Option<f64> {
    let value = ext_param.as_ref()?.get(key)?;
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// `data` from `/client/student/job/info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub job_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub study_task_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub content_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub paper_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub task_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub task_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub stu_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub begin_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub end_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub submit_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score: Option<f64>,
    #[serde(default)]
    pub raw: Value,
}

impl JobInfo {
    pub fn id(&self) -> Option<&str> {
        self.job_id.as_deref().or(self.study_task_id.as_deref())
    }

    pub fn finished(&self) -> bool {
        self.task_status == Some(1) || self.stu_status.is_some_and(|status| status > 1)
    }
}

/// One row from the account-wide finished-job list.
///
/// Rows are keyed by `studyTaskId`, which equals the job's `jobId` (not its
/// `contentId`). `jobId` itself only appears in older wrappers of the same
/// payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedJob {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub study_task_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub job_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub job_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub content_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub paper_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub task_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub stu_status: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub end_time: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub submit_time: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

impl FinishedJob {
    /// The job id this row marks as submitted.
    pub fn id(&self) -> Option<&str> {
        self.study_task_id.as_deref().or(self.job_id.as_deref())
    }
}

/// The account-wide finished and returned rows for one student.
#[derive(Debug, Clone, Default)]
pub struct FinishedJobs {
    /// `data.finishedList`.
    pub finished: Vec<FinishedJob>,
    /// `data.returnedList`.
    pub returned: Vec<FinishedJob>,
}

/// One row from `/client/book/finished-task/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedTask {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub task_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub chapter_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub batch_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub uid: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub max_score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub first_score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub percent: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub seconds: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub add_date: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub study_date: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

/// One node from `/client/student/chapter-task-score/list`.
///
/// The endpoint returns the book's chapters (`nodeType = 1`) and assignments
/// (`nodeType = 2`) as one flat array. `cid` is the same id the reader uses:
/// a task's `id` for an ordinary book, a content page's `id` for a type-2
/// course book. `taskType = 2` marks a vocabulary task; everything else is an
/// ordinary paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookTaskNode {
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub cid: String,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub pid: Option<String>,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub node_name: String,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub node_type: Option<i64>,
    /// `1` = ordinary paper, `2` = vocabulary (word-skill) assignment.
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub task_type: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub task_score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub percent: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub seconds: Option<i64>,
    #[serde(default)]
    pub raw: Value,
}

impl BookTaskNode {
    pub fn is_chapter(&self) -> bool {
        self.node_type == Some(1) && !self.cid.is_empty()
    }

    pub fn is_task(&self) -> bool {
        self.node_type == Some(2) && !self.cid.is_empty()
    }

    /// A vocabulary assignment, which the UI presents like a word-PK task.
    pub fn is_word_task(&self) -> bool {
        self.task_type == Some(2)
    }
}

/// One question row from `/client/question/student/task/question-score/list`.
///
/// This is the read-only detail the book reader shows for a finished or
/// unfinished task. Read-aloud questions (`questionTypeNum` 9/12/13) carry
/// the student's recording URL in `myAnswer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookQuestionRow {
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub question_id: String,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub question_no: String,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub qst_type_name: String,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub question_type_num: Option<i64>,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub knowledge: String,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub my_answer: String,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub correct_answer: String,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub question_score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub avg_score: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub score_rate: Option<f64>,
    #[serde(default)]
    pub raw: Value,
}

impl BookQuestionRow {
    /// The question types the reader renders as recordings.
    pub fn is_read_aloud(&self) -> bool {
        matches!(self.question_type_num, Some(9 | 12 | 13))
    }
}

/// One row from `/client/word/task/student/word-skill-get/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookWordRow {
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub word: String,
    #[serde(default, deserialize_with = "flex::opt_bool")]
    pub pass: Option<bool>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub degree: Option<i64>,
    #[serde(default)]
    pub raw: Value,
}

impl BookWordRow {
    pub fn mastered(&self) -> bool {
        self.pass == Some(true)
    }
}

/// A bookshelf entry, also used for `book_info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub goods_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub goods_img: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub cover_img: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_img: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub goods_vertical_img: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub vertical_cover: Option<String>,
    /// 1 = purchased/reader book, 2 = course-assigned book. `book_info` needs
    /// the real value; a type-2 book asked for as type 1 answers `data: null`.
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub book_type: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub pack_type: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub start_date: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub end_date: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub valid_days: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub stage_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub subject_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub category_type: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub file_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub pc_file_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub pc_version: Option<i64>,
    #[serde(default)]
    pub resource_list: Option<Value>,
    #[serde(default)]
    pub raw: Value,
}

impl Book {
    pub fn id(&self) -> Option<&str> {
        self.book_id.as_deref()
    }

    pub fn title(&self) -> &str {
        self.book_name
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("")
    }

    pub fn cover(&self) -> Option<&str> {
        self.goods_img
            .as_deref()
            .or(self.cover_img.as_deref())
            .or(self.book_img.as_deref())
    }

    pub fn vertical_cover(&self) -> Option<&str> {
        self.goods_vertical_img
            .as_deref()
            .or(self.vertical_cover.as_deref())
    }

    /// `packType == 9` selects the 2025 reader.
    pub fn is_2025_reader(&self) -> bool {
        self.pack_type == Some(9)
    }

    /// `bookType` to send back to the book endpoints.
    ///
    /// Purchased books are type 1 and class-assigned course books are type 2.
    /// The server answers `data: null` when a type-2 book is requested as type
    /// 1, so the shelf value has to be reused instead of hardcoding it.
    pub fn book_type(&self) -> i64 {
        self.book_type.unwrap_or(1)
    }
}

/// One row from `/client/book/plan`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookPlanEntry {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub chapter_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub chapter_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_bool")]
    pub is_close: Option<bool>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub book_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_id: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

impl BookPlanEntry {
    /// A closed chapter is skipped when building the reading tree.
    pub fn is_closed(&self) -> bool {
        self.is_close.unwrap_or(false)
    }
}

/// Parameters for `answer_detail`.
///
/// `task_id` must be the job's `contentId`, not its `jobId`: passing `jobId`
/// yields an empty `paperInfo` upstream.
#[derive(Debug, Clone)]
pub struct AnswerDetailParams {
    pub course_id: String,
    pub uid: String,
    pub book_id: String,
    pub task_id: String,
}

impl AnswerDetailParams {
    pub fn new(
        course_id: impl Into<String>,
        uid: impl Into<String>,
        book_id: impl Into<String>,
        content_id: impl Into<String>,
    ) -> Self {
        Self {
            course_id: course_id.into(),
            uid: uid.into(),
            book_id: book_id.into(),
            task_id: content_id.into(),
        }
    }
}

/// `data` from either version of `/client/student/task/answer/detail`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDetail {
    #[serde(default)]
    pub paper_info: Option<Value>,
    #[serde(default)]
    pub user_answer: Option<Value>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub batch_id: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub course_name: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_f64")]
    pub percent: Option<f64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub seconds: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub study_date: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub submit_date: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub task_name: Option<String>,
    #[serde(default)]
    pub raw: Value,
}

impl AnswerDetail {
    pub fn paper_data(&self) -> Option<&str> {
        nested_str(&self.paper_info, "paperData")
    }

    pub fn paper_answer_data(&self) -> Option<&str> {
        nested_str(&self.paper_info, "answerData")
    }

    pub fn user_answer_data(&self) -> Option<&str> {
        nested_str(&self.user_answer, "answerData")
    }

    pub fn mark_data(&self) -> Option<&str> {
        nested_str(&self.user_answer, "markData")
    }

    pub fn score(&self) -> Option<f64> {
        nested_f64(&self.user_answer, "score").or_else(|| nested_f64(&self.paper_info, "score"))
    }
}

/// `data` from `/client/paper/detail`; `paper_data` is an XML string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperDetail {
    #[serde(
        default,
        rename = "paperData",
        deserialize_with = "flex::string_or_empty"
    )]
    pub paper_data: String,
    #[serde(default)]
    pub raw: Value,
}

impl PaperDetail {
    pub fn as_str(&self) -> &str {
        &self.paper_data
    }

    pub fn is_empty(&self) -> bool {
        self.paper_data.is_empty()
    }
}

/// One word entry from a word-PK bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub entry_id: Option<String>,
    /// Older buckets omit `entryId` and identify the word by `pkid` instead.
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub pkid: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub entry: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub paraphrase: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub us_phonetic: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub en_phonetic: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub us_voice_file: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_string")]
    pub en_voice_file: Option<String>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub source_type: Option<i64>,
    #[serde(default)]
    pub new_prop_list: Value,
    #[serde(default)]
    pub raw: Value,
}

/// Base URL for word audio files.
pub const WORD_AUDIO_BASE: &str = "https://word-cdn-alyun.up366.cn";

impl Word {
    /// The id the server identifies this word by.
    pub fn id(&self) -> Option<&str> {
        self.entry_id
            .as_deref()
            .or(self.pkid.as_deref())
            .filter(|id| !id.is_empty())
    }

    /// Display translation, preferring `newPropList` over `paraphrase`.
    pub fn cn(&self) -> String {
        let mut parts = Vec::new();
        if let Some(props) = self.new_prop_list.as_array() {
            for prop in props {
                let mut text = prop
                    .get("prop")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let mut has_sense = false;
                if let Some(senses) = prop.get("senseList").and_then(Value::as_array) {
                    for sense in senses {
                        if sense.get("senseType").and_then(Value::as_i64) == Some(0) {
                            if let Some(paraphrase) =
                                sense.get("paraphrase").and_then(Value::as_str)
                            {
                                text.push_str(paraphrase);
                                has_sense = true;
                            }
                        }
                    }
                }
                if has_sense {
                    parts.push(text);
                }
            }
        }
        if parts.is_empty() {
            self.paraphrase.clone().unwrap_or_default()
        } else {
            parts.join("；")
        }
    }

    /// Audio URL, preferring the US voice.
    pub fn audio_url(&self) -> String {
        let name = self
            .us_voice_file
            .as_deref()
            .or(self.en_voice_file.as_deref())
            .unwrap_or("");
        if name.is_empty() {
            String::new()
        } else if name.starts_with("http") {
            name.to_owned()
        } else {
            format!("{WORD_AUDIO_BASE}/{name}")
        }
    }
}

/// Paging metadata (`data.pager`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pager {
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub current_page: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub page_size: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub total_count: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub total_page: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub index_page_count: Option<i64>,
    #[serde(default, deserialize_with = "flex::opt_i64")]
    pub offset: Option<i64>,
    #[serde(default)]
    pub raw: Value,
}

impl Pager {
    pub fn from_object(object: Map<String, Value>) -> Self {
        let raw = Value::Object(object.clone());
        let mut pager: Pager = serde_json::from_value(raw.clone()).unwrap_or_default();
        pager.raw = raw;
        pager
    }

    /// Whether another page follows. `currentPage` is 1-based upstream.
    pub fn has_next(&self) -> bool {
        let Some(current) = self.current_page else {
            return false;
        };
        if let Some(total_page) = self.total_page {
            return current < total_page;
        }
        match (self.page_size, self.total_count) {
            (Some(size), Some(total)) if size > 0 => current * size < total,
            _ => false,
        }
    }
}

impl RawPayload for Pager {
    fn set_raw(&mut self, raw: Value) {
        self.raw = raw;
    }
}

/// One page of decoded items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    #[serde(default)]
    pub pager: Pager,
}

impl<T> Page<T> {
    pub fn into_items(self) -> Vec<T> {
        self.items
    }
}

fn nested_str<'a>(value: &'a Option<Value>, key: &str) -> Option<&'a str> {
    value.as_ref()?.get(key)?.as_str()
}

fn nested_f64(value: &Option<Value>, key: &str) -> Option<f64> {
    match value.as_ref()?.get(key)? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

impl_raw!(
    Course,
    Job,
    JobInfo,
    FinishedJob,
    FinishedTask,
    BookTaskNode,
    BookQuestionRow,
    BookWordRow,
    Book,
    BookPlanEntry,
    AnswerDetail,
    PaperDetail,
    Word,
);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn job_folds_ext_param_and_classifies_word_pk() {
        let raw = json!({
            "jobId": "JOB-1",
            "jobName": "单词卡 Unit 1",
            "contentId": "CONTENT-1",
            "extParam": "{\"bookId\":\"BOOK-1\",\"paperId\":\"P-1\",\"taskStatus\":1,\"xotParam\":\"{\\\"pkBucketId\\\":\\\"B-1\\\"}\"}"
        });
        let job = Job::from_value(raw.clone()).unwrap();

        assert_eq!(job.id(), Some("JOB-1"));
        assert_eq!(job.book(), Some("BOOK-1"));
        assert_eq!(job.paper(), Some("P-1"));
        assert_eq!(job.task(), Some("CONTENT-1"));
        assert_eq!(job.status(), Some(1));
        assert!(job.finished());
        assert!(job.is_word_pk());
        assert_eq!(job.kind(), up366_core::JobKind::WordPk);
        assert_eq!(job.raw, raw);
    }

    #[test]
    fn ordinary_job_is_only_a_word_pk_hint_when_titled_so() {
        let job = Job::from_value(json!({
            "jobId": "JOB-2",
            "jobName": "Unit 3 阅读理解",
            "extParam": {}
        }))
        .unwrap();
        assert!(!job.is_word_pk());
        assert_eq!(job.kind(), up366_core::JobKind::Unknown);
    }

    #[test]
    fn read_aloud_hint_uses_the_title_and_open_type() {
        let by_name = Job::from_value(json!({
            "jobId": "JOB-3",
            "jobName": "Unit 4/课文1/逐句跟读",
            "extParam": {}
        }))
        .unwrap();
        assert!(by_name.is_read_aloud_hint());

        let by_open_type = Job::from_value(json!({
            "jobId": "JOB-4",
            "jobName": "Unit 4",
            "extParam": "{\"openType\":\"ListenAndReadv1\"}"
        }))
        .unwrap();
        assert!(by_open_type.is_read_aloud_hint());

        let ordinary = Job::from_value(json!({
            "jobId": "JOB-5",
            "jobName": "Unit 3 阅读理解",
            "extParam": {}
        }))
        .unwrap();
        assert!(!ordinary.is_read_aloud_hint());
    }

    #[test]
    fn word_translation_and_audio_follow_reference_rules() {
        let word: Word = serde_json::from_value(json!({
            "entryId": 7,
            "entry": "apple",
            "paraphrase": "fallback",
            "usVoiceFile": "apple_us.mp3",
            "newPropList": [
                { "prop": "n.", "senseList": [{ "senseType": 0, "paraphrase": "苹果" }] },
                { "prop": "忽略", "senseList": [{ "senseType": 1, "paraphrase": "x" }] }
            ]
        }))
        .unwrap();

        assert_eq!(word.entry_id.as_deref(), Some("7"));
        assert_eq!(word.cn(), "n.苹果");
        assert_eq!(
            word.audio_url(),
            "https://word-cdn-alyun.up366.cn/apple_us.mp3"
        );
    }

    #[test]
    fn word_falls_back_to_paraphrase_and_english_audio() {
        let word: Word = serde_json::from_value(json!({
            "paraphrase": "苹果",
            "enVoiceFile": "https://cdn/apple.mp3"
        }))
        .unwrap();
        assert_eq!(word.cn(), "苹果");
        assert_eq!(word.audio_url(), "https://cdn/apple.mp3");
    }

    #[test]
    fn pager_has_next_handles_both_shapes() {
        let pager = Pager::from_object(
            json!({ "currentPage": 2, "pageSize": 10, "totalCount": 25 })
                .as_object()
                .cloned()
                .unwrap(),
        );
        assert!(pager.has_next());

        let last = Pager::from_object(
            json!({ "currentPage": 3, "pageSize": 10, "totalCount": 25 })
                .as_object()
                .cloned()
                .unwrap(),
        );
        assert!(!last.has_next());
    }
}
