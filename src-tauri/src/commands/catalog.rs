//! Catalog commands: courses and the merged job list.

use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use serde::Deserialize;
use up366_core::AccountId;
use up366_protocol::{adapters, dto::Course, Client};
use up366_store::{CourseRecord, JobRecord};

use crate::error::CommandResult;
use crate::session::AppState;
use crate::state::{CourseView, FinishedIndex, JobView};

/// Optional filter for `jobs_list`, kept in one object so the command signature
/// stays stable as filters grow.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct JobFilter {
    /// Only rows whose name contains this text.
    #[serde(default)]
    pub query: Option<String>,
    /// One of `finished` / `pending` / `returned` / `expired` / `unknown`.
    #[serde(default)]
    pub status: Option<String>,
}

/// The full course list, with the class label already split out.
#[tauri::command(rename_all = "snake_case")]
pub async fn courses_list(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<CourseView>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let courses = adapters::courses(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    Ok(courses
        .iter()
        .enumerate()
        .map(|(index, course)| course_view(course, index))
        .collect())
}

/// The flat, cross-class job list.
///
/// Every course is fetched in upstream order and its jobs appended in the order
/// the server returned them: the list is never grouped by class and never
/// re-sorted. A job that appears in two classes stays two rows.
#[tauri::command(rename_all = "snake_case")]
pub async fn jobs_list(
    account_id: String,
    refresh: Option<bool>,
    filter: Option<JobFilter>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<JobView>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let _ = refresh;
    let mut rows = refresh_catalog(&state, &account_id, &client).await?;

    if let Some(filter) = filter {
        rows.retain(|row| row_matches(row, &filter));
    }
    Ok(rows)
}

/// Fetch the catalog and persist it before returning the rows.
///
/// The answer and word-PK views resolve `contentId` / `bookId` from the cached
/// job row, so the list is also the write path for that cache. A course that
/// fails transiently is skipped, but a rejected session stops the refresh.
pub(crate) async fn refresh_catalog(
    state: &AppState,
    account_id: &AccountId,
    client: &Client,
) -> CommandResult<Vec<JobView>> {
    let courses = adapters::courses(client)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;

    let fetched_at = up366_store::now_millis();
    let course_records: Vec<CourseRecord> = courses
        .iter()
        .enumerate()
        .filter_map(|(index, course)| {
            Some(CourseRecord {
                course_id: up366_core::CourseId::from(course.id()?),
                name: course.name().to_owned(),
                order_index: index as i64,
                raw_json: serde_json::to_string(&course.raw).unwrap_or_default(),
                fetched_at,
            })
        })
        .collect();
    state
        .db
        .cache()
        .upsert_courses(account_id, &course_records)
        .await
        .map_err(storage_error)?;

    // The finished/returned rows are account-wide: the endpoint ignores
    // `courseId` and returns the same rows for every course, so one call per
    // refresh covers them all.
    let finished = match courses.first().and_then(Course::id) {
        Some(course_id) => FinishedIndex::new(
            &adapters::course_finished_jobs(client, course_id)
                .await
                .map_err(|error| state.session_error(account_id, client, error))?,
        ),
        None => FinishedIndex::default(),
    };
    let now = Utc::now();

    let mut rows: Vec<JobView> = Vec::new();
    let mut job_records: Vec<JobRecord> = Vec::new();
    for (index, course) in courses.iter().enumerate() {
        let Some(course_id) = course.id() else {
            continue;
        };
        let view = course_view(course, index);

        // One course failing must not empty the whole list, but a rejected
        // session is not a course failure: stop and ask for the password.
        let jobs = match adapters::jobs(client, course_id).await {
            Ok(jobs) => jobs,
            Err(error) if client.is_expired() => {
                return Err(state.session_error(account_id, client, error));
            }
            Err(_) => continue,
        };
        for (order, job) in jobs.iter().enumerate() {
            let job_state = finished.state(job, now);
            let score = match job_state {
                up366_core::JobState::Finished { score } => score,
                _ => None,
            };
            rows.push(JobView::new_with_state(
                job,
                view.name.clone(),
                view.label().to_owned(),
                score,
                job.is_read_aloud_hint(),
                Some(job_state),
            ));
            if let Some(record) = job_record(course_id, job, order, fetched_at) {
                job_records.push(record);
            }
        }
    }

    state
        .db
        .cache()
        .upsert_jobs(account_id, &job_records)
        .await
        .map_err(storage_error)?;
    Ok(rows)
}

/// One job as it is cached for the detail views.
///
/// The answer endpoint needs `contentId` and `bookId`, which only the job
/// payload carries; keeping them here is what lets `answer_get` resolve a job
/// without a second upstream lookup.
fn job_record(
    course_id: &str,
    job: &up366_protocol::dto::Job,
    order: usize,
    fetched_at: i64,
) -> Option<JobRecord> {
    Some(JobRecord {
        course_id: up366_core::CourseId::from(course_id),
        job_id: up366_core::JobId::from(job.id()?),
        content_id: job.content().map(str::to_owned),
        book_id: job.book().map(str::to_owned),
        chapter_id: job.chapter_id.clone(),
        paper_id: job.paper().map(str::to_owned),
        name: job.title().to_owned(),
        order_index: order as i64,
        raw_json: serde_json::to_string(&job.raw).unwrap_or_default(),
        fetched_at,
    })
}

fn storage_error(error: up366_store::StoreError) -> crate::error::AppError {
    crate::error::AppError::new("storage_error", error.to_string())
}

/// Local-only filtering; ordering is untouched.
fn row_matches(row: &JobView, filter: &JobFilter) -> bool {
    if let Some(query) = filter.query.as_deref() {
        if !row.name.contains(query) {
            return false;
        }
    }
    if let Some(status) = filter.status.as_deref() {
        use crate::state::StatusView;
        let actual = match row.status {
            StatusView::Finished { .. } => "finished",
            StatusView::Returned => "returned",
            StatusView::Expired => "expired",
            StatusView::Pending => "pending",
            StatusView::Unknown => "unknown",
        };
        if actual != status {
            return false;
        }
    }
    true
}

/// Map a decoded course into the view that labels job rows.
fn course_view(course: &Course, order: usize) -> CourseView {
    let name = course.name().to_owned();
    let (class_name, teacher_name) = split_course_name(&name);
    CourseView {
        course_id: course.id().unwrap_or_default().to_owned(),
        order,
        class_name,
        teacher_name,
        name,
    }
}

/// The class label shown next to a job.
///
/// Upstream course names usually end with the class, for example
/// `英语必修一2028届17班`. The class is the tail starting with a year, so it is
/// extracted rather than invented; when nothing matches, the full name is the
/// label.
fn split_course_name(name: &str) -> (Option<String>, Option<String>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"20\d{2}届.{1,20}$").expect("static regex compiles"));
    match name.find("20").and_then(|start| {
        let tail = &name[start..];
        re.find(tail).map(|found| (start, found.end() + start))
    }) {
        Some((start, end)) => {
            let class = name[start..end].trim().to_owned();
            let subject = name[..start].trim();
            let teacher = (!subject.is_empty()).then(|| subject.to_owned());
            (Some(class), teacher)
        }
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn course(name: &str) -> Course {
        serde_json::from_value(json!({"courseId": "1", "courseName": name})).unwrap()
    }

    #[test]
    fn class_is_split_off_the_course_name() {
        let view = course_view(&course("英语必修一2028届17班"), 3);
        assert_eq!(view.order, 3);
        assert_eq!(view.label(), "2028届17班");
        assert_eq!(view.teacher_name.as_deref(), Some("英语必修一"));
    }

    #[test]
    fn a_course_without_a_year_marker_has_no_class() {
        let view = course_view(&course("数学"), 0);
        assert_eq!(view.class_name, None);
        assert_eq!(view.teacher_name, None);
        // The whole name is still used, so rows are never unlabelled.
        assert_eq!(view.label(), "数学");
    }

    #[test]
    fn a_class_marker_at_the_start_leaves_no_subject() {
        let view = course_view(&course("2028届17班"), 0);
        assert_eq!(view.label(), "2028届17班");
        assert_eq!(view.teacher_name, None);
    }

    #[test]
    fn a_cached_job_keeps_the_ids_the_answer_view_needs() {
        let job = up366_protocol::dto::Job::from_value(json!({
            "jobId": "JOB-1",
            "jobName": "Unit 4",
            "contentId": "CONTENT-1",
            "extParam": { "bookId": "BOOK-1", "chapterId": "CH-1", "taskStatus": 0 }
        }))
        .unwrap();

        let record = job_record("COURSE-1", &job, 2, 99).unwrap();

        // The job id is not the task id: the answer endpoint needs contentId.
        assert_eq!(record.job_id.as_str(), "JOB-1");
        assert_eq!(record.content_id.as_deref(), Some("CONTENT-1"));
        assert_eq!(record.book_id.as_deref(), Some("BOOK-1"));
        assert_eq!(record.chapter_id.as_deref(), Some("CH-1"));
        assert_eq!(record.name, "Unit 4");
        assert_eq!(record.order_index, 2);
        assert_eq!(record.fetched_at, 99);
    }

    #[test]
    fn query_and_status_filters_narrow_the_list_without_reordering_it() {
        use crate::state::StatusView;
        let make = |job_id: &str, name: &str, status: StatusView| JobView {
            job_id: job_id.to_owned(),
            course_id: "1".to_owned(),
            course_name: "数学".to_owned(),
            class_name: "一班".to_owned(),
            name: name.to_owned(),
            kind: up366_core::JobKind::Unknown,
            status,
            score: None,
            deadline: None,
            read_aloud: false,
            audio_pending: false,
        };
        let finished = make("a", "Unit 1", StatusView::Finished { score: Some(90.0) });
        let pending = make("b", "Unit 2", StatusView::Pending);

        assert!(row_matches(&finished, &JobFilter::default()));
        assert!(row_matches(
            &finished,
            &JobFilter {
                query: Some("Unit".to_owned()),
                status: None
            }
        ));
        assert!(!row_matches(
            &finished,
            &JobFilter {
                query: Some("Other".to_owned()),
                status: None
            }
        ));
        assert!(!row_matches(
            &finished,
            &JobFilter {
                query: None,
                status: Some("pending".to_owned())
            }
        ));
        assert!(row_matches(
            &pending,
            &JobFilter {
                query: None,
                status: Some("pending".to_owned())
            }
        ));
    }
}
