//! Catalog commands: courses and the merged job list.

use std::sync::OnceLock;

use chrono::Utc;
use regex::Regex;
use serde::Deserialize;
use up366_protocol::{adapters, dto::Course};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;
use crate::state::{CourseView, JobView};

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
#[tauri::command]
pub async fn courses_list(state: tauri::State<'_, AppState>) -> CommandResult<Vec<CourseView>> {
    let (_, client) = state.require_client()?;
    let courses = adapters::courses(&client).await.map_err(AppError::from)?;
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
#[tauri::command]
pub async fn jobs_list(
    filter: Option<JobFilter>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<JobView>> {
    let (_, client) = state.require_client()?;
    let courses = adapters::courses(&client).await.map_err(AppError::from)?;

    let mut rows: Vec<JobView> = Vec::new();
    for (index, course) in courses.iter().enumerate() {
        let Some(course_id) = course.id() else {
            continue;
        };
        let view = course_view(course, index);

        // One course failing must not empty the whole list.
        let Ok(jobs) = adapters::jobs(&client, course_id).await else {
            continue;
        };
        for job in &jobs {
            let state = crate::state::resolve_job_state(
                None,
                false,
                job.status(),
                job.student_status(),
                job.end_time.as_deref(),
                Utc::now(),
            );
            rows.push(JobView::new_with_state(
                job,
                view.name.clone(),
                view.label().to_owned(),
                None,
                false,
                Some(state),
            ));
        }
    }

    if let Some(filter) = filter {
        rows.retain(|row| row_matches(row, &filter));
    }
    Ok(rows)
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
