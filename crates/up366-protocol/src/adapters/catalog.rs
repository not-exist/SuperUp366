//! Course / job / finished-task adapters.

use serde_json::{json, Value};

use crate::adapters::{decode_items, endpoint};
use crate::client::Client;
use crate::codec;
use crate::dto::{self, Course, FinishedTask, Job, JobInfo};
use crate::error::{ProtocolError, Result};

/// `data.list` from `/client/course/list-of-student/v2`.
pub async fn courses(client: &Client) -> Result<Vec<Course>> {
    let endpoint = endpoint("courses")?;
    let data = client
        .call(
            endpoint,
            &[("status", json!(1)), ("pager###pageSize", json!(999))],
        )
        .await?;
    let mut object = codec::into_object(data, endpoint.name)?;
    let items = codec::take_list(&mut object, "list", endpoint.name)?;
    decode_items(items, endpoint.name)
}

/// `data.allTaskList` from `/client/student/course/job`.
pub async fn jobs(client: &Client, course_id: &str) -> Result<Vec<Job>> {
    let endpoint = endpoint("jobs")?;
    let data = client
        .call(
            endpoint,
            &[
                ("courseId", json!(course_id)),
                ("pageIdFlag", json!(1)),
                ("version", json!("")),
            ],
        )
        .await?;
    let mut object = codec::into_object(data, endpoint.name)?;
    let items = codec::take_list(&mut object, "allTaskList", endpoint.name)?;
    items
        .into_iter()
        .map(Job::from_value)
        .collect::<Result<Vec<_>>>()
}

/// `data` from `/client/student/all/course/range/job`.
///
/// The official H5 uses this endpoint for its "history" tab. It returns a flat
/// array of job rows in the same shape as `allTaskList`, including assignments
/// that `course/job` has already dropped, for every course the student belongs
/// to. `courseId` is accepted but ignored upstream, so callers should invoke it
/// once per refresh with a wide date range.
pub async fn history_jobs(
    client: &Client,
    course_id: Option<&str>,
    begin_time: &str,
    end_time: &str,
) -> Result<Vec<Job>> {
    let endpoint = endpoint("range_jobs")?;
    let mut fields: Vec<(&str, Value)> = Vec::with_capacity(3);
    if let Some(course_id) = course_id {
        fields.push(("courseId", json!(course_id)));
    }
    fields.push(("beginTime", json!(begin_time)));
    fields.push(("endTime", json!(end_time)));
    let data = client.call(endpoint, &fields).await?;
    let items = match data {
        None => Vec::new(),
        Some(data) => codec::into_array(Some(data), endpoint.name)?,
    };
    items.into_iter().map(Job::from_value).collect()
}

/// `data` from `/client/student/job/info`.
pub async fn job_info(
    client: &Client,
    course_id: &str,
    job_id: &str,
    content_id: &str,
    book_id: &str,
) -> Result<JobInfo> {
    let endpoint = endpoint("job_info")?;
    let data = client
        .call(
            endpoint,
            &[
                ("courseId", json!(course_id)),
                ("jobId", json!(job_id)),
                ("studyTaskId", json!(job_id)),
                ("contentId", json!(content_id)),
                ("bookId", json!(book_id)),
            ],
        )
        .await?;
    dto::decode(codec::require_data(data, endpoint.name)?, endpoint.name)
}

/// Finished and returned jobs for the signed-in student.
///
/// The endpoint is account-wide: it ignores `courseId` and returns the same
/// rows for every course. Callers must therefore invoke it once per refresh
/// (passing any course the student belongs to) and key the rows on
/// `studyTaskId`, which equals the job's `jobId`.
///
/// The payload is an object with `finishedList` / `returnedList`; an array or
/// the older `allTaskList` / `finishedJobList` / `list` wrapper is still
/// accepted as a single finished list.
pub async fn course_finished_jobs(client: &Client, course_id: &str) -> Result<dto::FinishedJobs> {
    let endpoint = endpoint("course_finished_jobs")?;
    let data = client
        .call(endpoint, &[("courseId", json!(course_id))])
        .await?;

    let mut rows = dto::FinishedJobs::default();
    match data {
        None => {}
        Some(Value::Array(items)) => {
            rows.finished = decode_items(items, endpoint.name)?;
        }
        Some(Value::Object(mut object)) => {
            if object.contains_key("finishedList") || object.contains_key("returnedList") {
                rows.finished = decode_items(
                    codec::take_list(&mut object, "finishedList", endpoint.name)?,
                    endpoint.name,
                )?;
                rows.returned = decode_items(
                    codec::take_list(&mut object, "returnedList", endpoint.name)?,
                    endpoint.name,
                )?;
            } else {
                let key = ["allTaskList", "finishedJobList", "list"]
                    .into_iter()
                    .find(|key| object.contains_key(*key))
                    .ok_or_else(|| {
                        ProtocolError::Decode(format!(
                            "{}: data has no finished-job list",
                            endpoint.name
                        ))
                    })?;
                rows.finished = decode_items(
                    codec::take_list(&mut object, key, endpoint.name)?,
                    endpoint.name,
                )?;
            }
        }
        Some(_) => {
            return Err(ProtocolError::Decode(format!(
                "{}: data is not an array or object",
                endpoint.name
            )))
        }
    }
    Ok(rows)
}

/// `data` from `/client/book/finished-task/list`.
///
/// A missing `data` member means "no finished tasks" upstream, so it decodes
/// to an empty list.
pub async fn finished_tasks(client: &Client, book_id: &str) -> Result<Vec<FinishedTask>> {
    let endpoint = endpoint("finished_tasks")?;
    let data = client.call(endpoint, &[("bookId", json!(book_id))]).await?;
    let items = if data.is_none() {
        Vec::new()
    } else {
        codec::into_array(data, endpoint.name)?
    };
    decode_items(items, endpoint.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    #[tokio::test]
    async fn courses_posts_expected_form_and_decodes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/course/list-of-student/v2")
                .body_includes("status=1")
                .body_includes("pager.pageSize=999");
            then.status(200).json_body(ok(json!({
                "list": [{ "courseId": 9222783, "courseName": "2028届17班", "status": 1 }]
            })));
        });

        let client = client(&server);
        let courses = courses(&client).await.unwrap();

        assert_eq!(courses.len(), 1);
        assert_eq!(courses[0].id(), Some("9222783"));
        assert_eq!(courses[0].name(), "2028届17班");
        assert_eq!(courses[0].raw["status"], json!(1));
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn courses_reject_missing_data() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/course/list-of-student/v2");
            then.status(200)
                .json_body(json!({ "result": { "code": 0, "msg": "ok" } }));
        });

        let client = client(&server);
        let error = courses(&client).await.unwrap_err();
        assert!(error.to_string().contains("did not contain data"));
    }

    #[tokio::test]
    async fn jobs_reads_all_task_list_and_folds_ext_param() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/course/job")
                .body_includes("courseId=9222783")
                .body_includes("pageIdFlag=1")
                .body_includes("version=");
            then.status(200).json_body(ok(json!({
                "allTaskList": [{
                    "jobId": "JOB-1",
                    "jobName": "Unit 4",
                    "contentId": "CONTENT-1",
                    "extParam": "{\"bookId\":\"BOOK-1\",\"taskStatus\":1}"
                }],
                "ownerTaskList": [{ "jobId": "IGNORED" }]
            })));
        });

        let client = client(&server);
        let jobs = jobs(&client, "9222783").await.unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id(), Some("JOB-1"));
        assert_eq!(jobs[0].book(), Some("BOOK-1"));
        assert_eq!(jobs[0].task(), Some("CONTENT-1"));
        assert!(jobs[0].finished());
        assert_eq!(jobs[0].raw["jobId"], json!("JOB-1"));
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn history_jobs_posts_the_date_range_and_decodes_old_rows() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/range/job")
                .body_includes("courseId=9073115")
                .body_includes("beginTime=2000-01-01")
                .body_includes("endTime=2026-11-16");
            then.status(200).json_body(ok(json!([{
                "jobId": "OLD-1",
                "jobName": "2025 homework",
                "courseId": 9073115,
                "beginTime": "1756396800000",
                "endTime": "1757433600000",
                "extParam": "{\"bookId\":\"BOOK-OLD\",\"taskStatus\":1}"
            }])));
        });

        let client = client(&server);
        let jobs = history_jobs(&client, Some("9073115"), "2000-01-01", "2026-11-16")
            .await
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id(), Some("OLD-1"));
        assert_eq!(jobs[0].title(), "2025 homework");
        assert_eq!(jobs[0].course(), Some("9073115"));
        assert_eq!(jobs[0].book(), Some("BOOK-OLD"));
        assert!(jobs[0].finished());
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn history_jobs_treats_missing_data_as_empty() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/range/job");
            then.status(200).json_body(ok(json!(null)));
        });

        let client = client(&server);
        let jobs = history_jobs(&client, None, "2000-01-01", "2026-11-16")
            .await
            .unwrap();

        assert!(jobs.is_empty());
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn job_info_posts_exact_parameters() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/job/info")
                .body_includes("jobId=JOB-1")
                .body_includes("studyTaskId=JOB-1")
                .body_includes("contentId=CONTENT-1")
                .body_includes("bookId=BOOK-1");
            then.status(200).json_body(ok(json!({
                "jobId": "JOB-1",
                "paperId": "PAPER-1",
                "taskStatus": 1
            })));
        });

        let client = client(&server);
        let info = job_info(&client, "9222783", "JOB-1", "CONTENT-1", "BOOK-1")
            .await
            .unwrap();

        assert_eq!(info.id(), Some("JOB-1"));
        assert_eq!(info.paper_id.as_deref(), Some("PAPER-1"));
        assert!(info.finished());
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn course_finished_jobs_decodes_finished_and_returned_lists() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/finish/job/v2")
                .body_includes("courseId=9222783");
            then.status(200).json_body(ok(json!({
                "studyPlanList": [{ "studyPlanId": 1, "flevel": 0 }],
                "finishedList": [{
                    "studyTaskId": "JOB-1",
                    "courseId": 9222783,
                    "batchId": "B-1",
                    "score": 30.0,
                    "jobScore": "30.0分",
                    "studyDate": 1783151094000_i64
                }],
                "returnedList": [{
                    "studyTaskId": "JOB-2",
                    "courseId": 9222783,
                    "score": null
                }],
                "complexFinishedList": [],
                "version": "V-1"
            })));
        });

        let client = client(&server);
        let rows = course_finished_jobs(&client, "9222783").await.unwrap();

        assert_eq!(rows.finished.len(), 1);
        assert_eq!(rows.finished[0].id(), Some("JOB-1"));
        assert_eq!(rows.finished[0].score, Some(30.0));
        assert_eq!(rows.returned.len(), 1);
        assert_eq!(rows.returned[0].id(), Some("JOB-2"));
        assert_eq!(rows.returned[0].score, None);
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn course_finished_jobs_accepts_plain_and_legacy_wrappers() {
        let server = MockServer::start();
        let plain = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/finish/job/v2")
                .body_includes("courseId=2");
            then.status(200).json_body(ok(json!([{
                "studyTaskId": "JOB-2",
                "score": 12
            }])));
        });
        let wrapped = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/finish/job/v2")
                .body_includes("courseId=1");
            then.status(200).json_body(ok(json!({
                "allTaskList": [{ "jobId": "JOB-1", "jobName": "Done", "score": 90 }]
            })));
        });

        let client = client(&server);
        let plain_rows = course_finished_jobs(&client, "2").await.unwrap();
        assert_eq!(plain_rows.finished[0].id(), Some("JOB-2"));
        assert_eq!(plain_rows.finished[0].score, Some(12.0));

        let wrapped_rows = course_finished_jobs(&client, "1").await.unwrap();
        assert_eq!(wrapped_rows.finished[0].id(), Some("JOB-1"));
        assert_eq!(wrapped_rows.finished[0].score, Some(90.0));
        plain.assert_calls(1);
        wrapped.assert_calls(1);
    }

    #[tokio::test]
    async fn finished_tasks_decodes_scores_and_tolerates_missing_data() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/book/finished-task/list")
                .body_includes("bookId=BOOK-1");
            then.status(200).json_body(ok(json!([{
                "taskId": "T-1",
                "bookId": "BOOK-1",
                "score": 8,
                "maxScore": 10,
                "percent": 0.8,
                "studyDate": "2026-10-01"
            }])));
        });
        let empty = server.mock(|when, then| {
            when.method(POST)
                .path("/client/book/finished-task/list")
                .body_includes("bookId=BOOK-EMPTY");
            then.status(200)
                .json_body(json!({ "result": { "code": 0, "msg": "ok" } }));
        });

        let client = client(&server);
        let tasks = finished_tasks(&client, "BOOK-1").await.unwrap();
        assert_eq!(tasks[0].task_id.as_deref(), Some("T-1"));
        assert_eq!(tasks[0].score, Some(8.0));
        assert_eq!(tasks[0].max_score, Some(10.0));
        assert_eq!(tasks[0].percent, Some(0.8));
        assert!(finished_tasks(&client, "BOOK-EMPTY")
            .await
            .unwrap()
            .is_empty());

        mock.assert_calls(1);
        empty.assert_calls(1);
    }
}
