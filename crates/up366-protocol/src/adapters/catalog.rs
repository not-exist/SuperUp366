//! Course / job / finished-task adapters.

use serde_json::{json, Value};

use crate::adapters::{decode_items, endpoint};
use crate::client::Client;
use crate::codec;
use crate::dto::{self, Course, FinishedJob, FinishedTask, Job, JobInfo};
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

/// Finished jobs for a whole course.
///
/// The only known client for this endpoint is the H5 bundle, whose response
/// wrapper is not documented with the same confidence as the others. Both an
/// array and the common `allTaskList` / `finishedJobList` / `list` wrappers
/// are accepted; anything else is a decode error.
pub async fn course_finished_jobs(client: &Client, course_id: &str) -> Result<Vec<FinishedJob>> {
    let endpoint = endpoint("course_finished_jobs")?;
    let data = client
        .call(endpoint, &[("courseId", json!(course_id))])
        .await?;

    let items = match data {
        None => Vec::new(),
        Some(Value::Array(items)) => items,
        Some(Value::Object(mut object)) => {
            let mut found = None;
            for key in ["allTaskList", "finishedJobList", "list"] {
                if object.contains_key(key) {
                    found = Some(codec::take_list(&mut object, key, endpoint.name)?);
                    break;
                }
            }
            found.ok_or_else(|| {
                ProtocolError::Decode(format!("{}: data has no finished-job list", endpoint.name))
            })?
        }
        Some(_) => {
            return Err(ProtocolError::Decode(format!(
                "{}: data is not an array or object",
                endpoint.name
            )))
        }
    };
    decode_items(items, endpoint.name)
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
                .body_contains("status=1")
                .body_contains("pager.pageSize=999");
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
        mock.assert_hits(1);
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
                .body_contains("courseId=9222783")
                .body_contains("pageIdFlag=1")
                .body_contains("version=");
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
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn job_info_posts_exact_parameters() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/job/info")
                .body_contains("jobId=JOB-1")
                .body_contains("studyTaskId=JOB-1")
                .body_contains("contentId=CONTENT-1")
                .body_contains("bookId=BOOK-1");
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
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn course_finished_jobs_accepts_wrapped_and_plain_lists() {
        let server = MockServer::start();
        let wrapped = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/all/course/finish/job/v2")
                .body_contains("courseId=1");
            then.status(200).json_body(ok(json!({
                "allTaskList": [{ "jobId": "JOB-1", "jobName": "Done", "score": 90 }]
            })));
        });

        let client = client(&server);
        let jobs = course_finished_jobs(&client, "1").await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id.as_deref(), Some("JOB-1"));
        assert_eq!(jobs[0].score, Some(90.0));
        wrapped.assert_hits(1);
    }

    #[tokio::test]
    async fn finished_tasks_decodes_scores_and_tolerates_missing_data() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/book/finished-task/list")
                .body_contains("bookId=BOOK-1");
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
                .body_contains("bookId=BOOK-EMPTY");
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

        mock.assert_hits(1);
        empty.assert_hits(1);
    }
}
