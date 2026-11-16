//! Catalog commands: courses and the merged job list.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use up366_core::AccountId;
use up366_protocol::{
    adapters,
    dto::{Course, Job},
    Client,
};
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
/// Live and historical jobs are merged across every class, then sorted newest
/// first by start time so older terms are reachable instead of falling off the
/// upstream list. A job that appears in two classes stays two rows.
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
/// `course/job` only carries live assignments: completed terms disappear from
/// `allTaskList` (course `9073115` answers zero rows although its 2025 jobs are
/// still on the account). The account-wide range endpoint — the official H5's
/// "history" tab — fills those rows back in, so both lists are merged by
/// `(courseId, jobId)`.
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

    let views: Vec<CourseView> = courses
        .iter()
        .enumerate()
        .map(|(index, course)| course_view(course, index))
        .collect();
    let view_index: HashMap<&str, usize> = views
        .iter()
        .enumerate()
        .map(|(index, view)| (view.course_id.as_str(), index))
        .collect();

    let mut catalog: Vec<CatalogJob> = Vec::new();

    for (index, course) in courses.iter().enumerate() {
        let Some(course_id) = course.id() else {
            continue;
        };
        let view = &views[index];

        // One course failing must not empty the whole list, but a rejected
        // session is not a course failure: stop and ask for the password.
        let jobs = match adapters::jobs(client, course_id).await {
            Ok(jobs) => jobs,
            Err(error) if client.is_expired() => {
                return Err(state.session_error(account_id, client, error));
            }
            Err(_) => continue,
        };
        for job in jobs {
            catalog.push(CatalogJob {
                course_id: course_id.to_owned(),
                course_name: view.name.clone(),
                class_name: view.label().to_owned(),
                course_order: index,
                job,
            });
        }
    }

    // History is one account-wide call; a transient failure must not hide the
    // live list, but a rejected session still stops the refresh.
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let history = adapters::history_jobs(
        client,
        courses.first().and_then(Course::id),
        HISTORY_BEGIN,
        &today,
    )
    .await;
    let history = match history {
        Ok(jobs) => jobs,
        Err(error) if client.is_expired() => {
            return Err(state.session_error(account_id, client, error));
        }
        Err(_) => Vec::new(),
    };
    append_history(&mut catalog, history, &views, &view_index);

    sort_newest_first(&mut catalog);

    let now = Utc::now();
    let mut rows: Vec<JobView> = Vec::with_capacity(catalog.len());
    let mut job_records: Vec<JobRecord> = Vec::with_capacity(catalog.len());
    for (order, item) in catalog.iter().enumerate() {
        let job_state = finished.state(&item.job, now);
        let score = match job_state {
            up366_core::JobState::Finished { score } => score,
            _ => None,
        };
        rows.push(JobView::new_with_state(
            &item.job,
            item.course_name.clone(),
            item.class_name.clone(),
            score,
            item.job.is_read_aloud_hint(),
            Some(job_state),
        ));
        if let Some(record) = job_record(&item.course_id, &item.job, order, fetched_at) {
            job_records.push(record);
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

/// One merged catalog row before it is mapped to the frontend view.
struct CatalogJob {
    course_id: String,
    course_name: String,
    class_name: String,
    /// Position of the course in the live course list, used only as a
    /// deterministic tie-break.
    course_order: usize,
    job: Job,
}

/// Wide enough for every account's history; the server clamps to its own data.
const HISTORY_BEGIN: &str = "2000-01-01";

/// Append history rows, skipping the ones the live list already has.
fn append_history(
    catalog: &mut Vec<CatalogJob>,
    history: impl IntoIterator<Item = Job>,
    views: &[CourseView],
    view_index: &HashMap<&str, usize>,
) {
    let mut seen: HashSet<(String, String)> = catalog
        .iter()
        .filter_map(|item| {
            item.job
                .id()
                .map(|id| (item.course_id.clone(), id.to_owned()))
        })
        .collect();
    for (offset, job) in history.into_iter().enumerate() {
        let Some(job_id) = job.id() else {
            continue;
        };
        let course_id = job.course().unwrap_or_default().to_owned();
        if !seen.insert((course_id.clone(), job_id.to_owned())) {
            continue;
        }
        let (course_name, class_name) = match view_index.get(course_id.as_str()) {
            Some(&index) => (views[index].name.clone(), views[index].label().to_owned()),
            None => {
                // The role of a retired course is not in the live list, but
                // every range row carries its own course name.
                let name = job
                    .raw
                    .get("courseName")
                    .and_then(|value| value.as_str())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&course_id)
                    .to_owned();
                let class = split_course_name(&name).0.unwrap_or_else(|| name.clone());
                (name, class)
            }
        };
        catalog.push(CatalogJob {
            course_id,
            course_name,
            class_name,
            // Keep the tie-break stable even when the course is not in the
            // live list: history rows come after live rows of equal time.
            course_order: views.len() + offset,
            job,
        });
    }
}

/// Newest first by start time, falling back to the deadline. Jobs without a
/// usable timestamp go last; equal times keep the live course order.
fn sort_newest_first(jobs: &mut [CatalogJob]) {
    jobs.sort_by(|left, right| {
        job_time(&right.job)
            .cmp(&job_time(&left.job))
            .then_with(|| left.course_order.cmp(&right.course_order))
            .then_with(|| left.course_id.cmp(&right.course_id))
            .then_with(|| left.job.title().cmp(right.job.title()))
            .then_with(|| {
                left.job
                    .id()
                    .unwrap_or_default()
                    .cmp(right.job.id().unwrap_or_default())
            })
    });
}

fn job_time(job: &Job) -> Option<DateTime<Utc>> {
    job.begin_time
        .as_deref()
        .and_then(up366_core::parse_deadline)
        .or_else(|| job.end_time.as_deref().and_then(up366_core::parse_deadline))
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

    fn catalog_job(
        id: &str,
        begin: Option<&str>,
        end: Option<&str>,
        course_order: usize,
    ) -> CatalogJob {
        let mut value = json!({ "jobId": id, "jobName": id });
        if let Some(begin) = begin {
            value["beginTime"] = json!(begin);
        }
        if let Some(end) = end {
            value["endTime"] = json!(end);
        }
        CatalogJob {
            course_id: "COURSE-1".to_owned(),
            course_name: "一班".to_owned(),
            class_name: "一班".to_owned(),
            course_order,
            job: Job::from_value(value).unwrap(),
        }
    }

    #[test]
    fn merged_catalog_is_newest_first_and_falls_back_to_the_deadline() {
        let mut jobs = vec![
            catalog_job("old-2025", Some("2025-09-01T00:00:00Z"), None, 0),
            catalog_job("new", Some("2026-11-01T00:00:00Z"), None, 0),
            catalog_job("no-time", None, None, 0),
            catalog_job("middle", Some("2026-01-01T00:00:00Z"), None, 0),
            catalog_job("deadline-only", None, Some("2025-12-01T00:00:00Z"), 0),
        ];

        sort_newest_first(&mut jobs);

        let ids: Vec<_> = jobs
            .iter()
            .map(|job| job.job.id().unwrap_or_default())
            .collect();
        assert_eq!(
            ids,
            vec!["new", "middle", "deadline-only", "old-2025", "no-time"]
        );
    }

    #[test]
    fn equal_times_keep_the_live_course_order() {
        let mut jobs = vec![
            catalog_job("second-course", Some("2026-01-01T00:00:00Z"), None, 1),
            catalog_job("first-course", Some("2026-01-01T00:00:00Z"), None, 0),
        ];

        sort_newest_first(&mut jobs);

        assert_eq!(jobs[0].job.id(), Some("first-course"));
        assert_eq!(jobs[1].job.id(), Some("second-course"));
    }

    fn history_job(course_id: &str, course_name: &str, job_id: &str) -> Job {
        Job::from_value(json!({
            "jobId": job_id,
            "jobName": job_id,
            "courseId": course_id,
            "courseName": course_name,
            "beginTime": "2025-09-01T00:00:00Z",
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn refresh_merges_live_and_history_and_caches_the_old_rows() {
        use std::time::Duration;

        use crate::state::StatusView;
        use httpmock::prelude::*;
        use up366_protocol::{Client, Session};

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .body_includes("status=1");
            then.status(200).json_body(json!({
                "result": { "code": 0, "msg": "ok" },
                "data": { "list": [{
                    "courseId": 9222783,
                    "courseName": "2028届17班",
                    "status": 1
                }] }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/course/job")
                .body_includes("courseId=9222783");
            then.status(200).json_body(json!({
                "result": { "code": 0, "msg": "ok" },
                "data": {
                    "allTaskList": [{
                        "jobId": "LIVE-1",
                        "jobName": "live homework",
                        "courseId": 9222783,
                        "beginTime": "2026-11-01T00:00:00Z",
                        "extParam": "{\"taskStatus\":0}"
                    }],
                    "ownerTaskList": []
                }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/finish/job/v2");
            then.status(200).json_body(json!({
                "result": { "code": 0, "msg": "ok" },
                "data": {
                    "finishedList": [{
                        "studyTaskId": "OLD-1",
                        "courseId": 9073115,
                        "score": 88.0
                    }],
                    "returnedList": []
                }
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/range/job")
                .body_includes("beginTime=2000-01-01");
            then.status(200).json_body(json!({
                "result": { "code": 0, "msg": "ok" },
                "data": [
                    {
                        "jobId": "LIVE-1",
                        "jobName": "live homework",
                        "courseId": 9222783,
                        "beginTime": "2026-11-01T00:00:00Z",
                        "extParam": "{\"taskStatus\":0}"
                    },
                    {
                        "jobId": "OLD-1",
                        "jobName": "2025 homework",
                        "courseId": 9073115,
                        "courseName": "2025级17班",
                        "beginTime": "2025-09-01T00:00:00Z",
                        "endTime": "2025-09-02T00:00:00Z",
                        "extParam": "{\"taskStatus\":1}"
                    },
                    {
                        "jobId": "OLD-2",
                        "jobName": "older homework",
                        "courseId": 9073115,
                        "courseName": "2025级17班",
                        "beginTime": "2025-08-01T00:00:00Z",
                        "endTime": "2025-08-02T00:00:00Z",
                        "extParam": "{\"taskStatus\":1}"
                    }
                ]
            }));
        });

        let client = Client::builder()
            .client_id("CID-TEST")
            .session(Session::new("alice", "TGT-1", "CID-TEST"))
            .base_url_override(server.base_url())
            .retry_base_delay(Duration::ZERO)
            .build()
            .unwrap();
        let db = up366_store::Database::memory().await.unwrap();
        let state = AppState::with_database(db);
        let account = AccountId::from("acct-1");
        state
            .db
            .accounts()
            .upsert(&up366_store::NewAccount::new(account.clone()))
            .await
            .unwrap();

        let rows = refresh_catalog(&state, &account, &client).await.unwrap();

        let ids: Vec<_> = rows.iter().map(|row| row.job_id.as_str()).collect();
        assert_eq!(ids, vec!["LIVE-1", "OLD-1", "OLD-2"]);
        assert_eq!(rows[1].course_name, "2025级17班");
        assert_eq!(rows[1].class_name, "2025级17班");
        assert!(matches!(
            rows[1].status,
            StatusView::Finished { score: Some(88.0) }
        ));
        assert!(matches!(
            rows[2].status,
            StatusView::Finished { score: None }
        ));

        let cached = state.db.cache().jobs(&account, None).await.unwrap();
        let cached_ids: Vec<_> = cached.iter().map(|row| row.job_id.as_str()).collect();
        assert!(cached_ids.contains(&"OLD-1"));
        assert!(cached_ids.contains(&"OLD-2"));
    }

    #[test]
    fn history_is_deduped_and_keeps_a_retired_courses_label() {
        let views = vec![CourseView {
            course_id: "LIVE".to_owned(),
            name: "2028届17班".to_owned(),
            order: 0,
            class_name: Some("2028届17班".to_owned()),
            teacher_name: None,
        }];
        let view_index = HashMap::from([("LIVE", 0usize)]);
        let mut catalog = vec![CatalogJob {
            course_id: "LIVE".to_owned(),
            course_name: "2028届17班".to_owned(),
            class_name: "2028届17班".to_owned(),
            course_order: 0,
            job: history_job("LIVE", "2028届17班", "SAME"),
        }];

        append_history(
            &mut catalog,
            vec![
                history_job("LIVE", "2028届17班", "SAME"),
                history_job("RETIRED", "2025级17班", "OLD"),
            ],
            &views,
            &view_index,
        );

        assert_eq!(catalog.len(), 2);
        let old = catalog
            .iter()
            .find(|item| item.job.id() == Some("OLD"))
            .unwrap();
        assert_eq!(old.course_id, "RETIRED");
        assert_eq!(old.course_name, "2025级17班");
        assert_eq!(old.class_name, "2025级17班");
    }
}
