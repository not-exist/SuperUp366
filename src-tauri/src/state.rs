//! Pure mappers from upstream DTOs to the frontend wire types.
//!
//! Everything the React app renders is derived here, in one place, from
//! [`up366_protocol::dto`] values plus the status rule in [`up366_core`]. The
//! Tauri commands are thin: fetch, map, return.

use serde::Serialize;
use up366_core::{JobKind, JobState};
use up366_protocol::dto::{Book, Job};

/// One stored account as the account switcher shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountView {
    pub account_id: String,
    pub username: String,
    pub display_name: String,
    pub school_name: String,
    pub active: bool,
    pub session_valid: bool,
    pub expires_at: Option<String>,
}

/// A course row, used by the course filter and to label jobs with their class.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CourseView {
    pub course_id: String,
    pub name: String,
    /// Upstream position, kept so the UI never reorders anything.
    pub order: usize,
    pub class_name: Option<String>,
    pub teacher_name: Option<String>,
}

impl CourseView {
    /// Best available class label for a course.
    pub fn label(&self) -> &str {
        self.class_name.as_deref().unwrap_or(&self.name)
    }
}

/// Job status payload. Mirrors `frontend/src/api/types.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "score")]
pub enum StatusView {
    Finished { score: Option<f64> },
    Returned,
    Expired,
    Pending,
    Unknown,
}

/// One row of the flat, cross-class job list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobView {
    pub job_id: String,
    pub course_id: String,
    pub course_name: String,
    pub class_name: String,
    pub name: String,
    pub kind: JobKind,
    pub status: StatusView,
    pub score: Option<f64>,
    pub deadline: Option<String>,
    pub read_aloud: bool,
    /// True when the paper must be fetched to know whether audio is needed.
    pub audio_pending: bool,
}

impl JobView {
    /// Map one upstream job, with the resolved status already computed.
    pub fn new(
        job: &Job,
        course_name: impl Into<String>,
        class_name: impl Into<String>,
        score: Option<f64>,
        read_aloud: bool,
    ) -> Self {
        Self::new_with_state(job, course_name, class_name, score, read_aloud, None)
    }

    /// Full form used by the sync layer, which already knows the job state.
    pub fn new_with_state(
        job: &Job,
        course_name: impl Into<String>,
        class_name: impl Into<String>,
        score: Option<f64>,
        read_aloud: bool,
        state: Option<JobState>,
    ) -> Self {
        let deadline = job.end_time.clone();
        let status = match state {
            Some(JobState::Finished { score }) => StatusView::Finished { score },
            Some(JobState::Returned) => StatusView::Returned,
            Some(JobState::Expired) => StatusView::Expired,
            Some(JobState::Pending) | None => StatusView::Pending,
            Some(JobState::Unknown) => StatusView::Unknown,
        };
        Self {
            job_id: job.id().unwrap_or_default().to_owned(),
            course_id: job.course().unwrap_or_default().to_owned(),
            course_name: course_name.into(),
            class_name: class_name.into(),
            name: job.title().to_owned(),
            kind: job.kind(),
            status,
            score: score.or(job.task_score),
            deadline,
            read_aloud,
            audio_pending: false,
        }
    }
}

/// Resolve a job's list status from raw signals, using the shared rule.
///
/// The wrapper exists so no caller repeats the deadline parsing and the
/// `StatusInputs` construction; the rule itself stays in `up366-core`.
pub fn resolve_job_state(
    finished_score: Option<f64>,
    in_finished_list: bool,
    task_status: Option<i64>,
    stu_status: Option<i64>,
    end_time: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> JobState {
    up366_core::resolve_status(&up366_core::StatusInputs {
        finished_score,
        in_finished_list,
        task_status,
        stu_status,
        end_time: end_time.and_then(up366_core::parse_deadline),
        detail_state: None,
        now,
    })
}

/// Shelf entry for the book picker.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookView {
    pub book_id: String,
    pub name: String,
    pub subject: String,
    pub cover_url: Option<String>,
    pub progress: u64,
    pub total: u64,
    pub status: &'static str,
}

impl BookView {
    /// Fields the shelf response does not carry start at zero.
    pub fn from_shelf(book: &Book) -> Self {
        Self {
            book_id: book.id().unwrap_or_default().to_owned(),
            name: book.title().to_owned(),
            subject: book.subject_id.clone().unwrap_or_default(),
            cover_url: book.cover().map(str::to_owned),
            progress: 0,
            total: 0,
            status: "active",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn job() -> Job {
        Job::from_value(serde_json::json!({
            "jobId": "JOB-1",
            "jobName": "Unit 4",
            "courseId": "9222783",
            "contentId": "CONTENT-1",
            "extParam": "{\"bookId\":\"BOOK-1\",\"taskStatus\":0}",
            "endTime": "2026-02-01T00:00:00Z",
        }))
        .unwrap()
    }

    #[test]
    fn job_view_carries_class_label_without_grouping() {
        let view = JobView::new(&job(), "英语一班", "一班", None, false);
        assert_eq!(view.class_name, "一班");
        assert_eq!(view.course_id, "9222783");
        assert_eq!(view.job_id, "JOB-1");
        assert_eq!(view.name, "Unit 4");
        // `contentId` is never used as the job id.
        assert_ne!(view.job_id, "CONTENT-1");
    }

    #[test]
    fn status_finished_reports_the_finished_list_score() {
        let now = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        let state = resolve_job_state(Some(93.5), true, Some(0), None, None, now);
        assert_eq!(state, JobState::Finished { score: Some(93.5) });
    }

    #[test]
    fn status_unfinished_after_deadline_is_expired() {
        let now = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        let state = resolve_job_state(
            None,
            false,
            Some(0),
            None,
            Some("2026-01-01T00:00:00Z"),
            now,
        );
        assert_eq!(state, JobState::Expired);
    }

    #[test]
    fn status_unfinished_before_deadline_is_pending() {
        let now = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        let state = resolve_job_state(
            None,
            false,
            Some(0),
            None,
            Some("2026-02-01T00:00:00Z"),
            now,
        );
        assert_eq!(state, JobState::Pending);
    }

    #[test]
    fn book_view_without_progress_starts_at_zero() {
        let book: Book = serde_json::from_value(serde_json::json!({
            "bookId": "BOOK-1",
            "bookName": "英语必修一"
        }))
        .unwrap();
        let view = BookView::from_shelf(&book);
        assert_eq!(view.book_id, "BOOK-1");
        assert_eq!(view.total, 0);
        assert_eq!(view.status, "active");
    }
}
