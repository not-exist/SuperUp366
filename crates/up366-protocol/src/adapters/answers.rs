//! Answer and paper-detail adapters.

use serde_json::json;

use crate::adapters::endpoint;
use crate::client::Client;
use crate::codec;
use crate::dto::{self, AnswerDetail, AnswerDetailParams, PaperDetail};
use crate::error::{ProtocolError, Result};

/// Prefer the v2 answer-detail endpoint, falling back to v1 when the route or
/// parameter shape is not available.
pub async fn answer_detail(client: &Client, params: &AnswerDetailParams) -> Result<AnswerDetail> {
    match answer_detail_v2(client, params).await {
        Ok(value) => Ok(value),
        Err(error) if should_fallback_to_v1(&error) => answer_detail_v1(client, params).await,
        Err(error) => Err(error),
    }
}

/// POST `/client/student/task/answer/detail/v2`.
pub async fn answer_detail_v2(
    client: &Client,
    params: &AnswerDetailParams,
) -> Result<AnswerDetail> {
    let endpoint = endpoint("answer_detail_v2")?;
    let data = client
        .call(
            endpoint,
            &[
                ("courseId", json!(&params.course_id)),
                ("uid", json!(&params.uid)),
                ("bookId", json!(&params.book_id)),
                // Must be contentId, not jobId.
                ("taskId", json!(&params.task_id)),
            ],
        )
        .await?;
    dto::decode(codec::require_data(data, endpoint.name)?, endpoint.name)
}

/// GET `/client/student/task/answer/detail`.
pub async fn answer_detail_v1(
    client: &Client,
    params: &AnswerDetailParams,
) -> Result<AnswerDetail> {
    let endpoint = endpoint("answer_detail_v1")?;
    let data = client
        .call(
            endpoint,
            &[
                ("courseId", json!(&params.course_id)),
                ("uid", json!(&params.uid)),
                ("bookId", json!(&params.book_id)),
                ("taskId", json!(&params.task_id)),
            ],
        )
        .await?;
    dto::decode(codec::require_data(data, endpoint.name)?, endpoint.name)
}

/// GET `/client/paper/detail`, whose `data.paperData` is an XML string.
pub async fn paper_detail(client: &Client, paper_id: &str) -> Result<PaperDetail> {
    let endpoint = endpoint("paper_detail")?;
    let data = client
        .call(endpoint, &[("paperId", json!(paper_id))])
        .await?;
    dto::decode(codec::require_data(data, endpoint.name)?, endpoint.name)
}

fn should_fallback_to_v1(error: &ProtocolError) -> bool {
    matches!(
        error,
        ProtocolError::Http {
            status: 404 | 405,
            ..
        } | ProtocolError::Api { code: -1, .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;
    use serde_json::json;

    fn params() -> AnswerDetailParams {
        AnswerDetailParams::new("COURSE-1", "39370562", "BOOK-1", "CONTENT-1")
    }

    #[tokio::test]
    async fn v2_is_preferred_and_uses_content_id() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/task/answer/detail/v2")
                .body_includes("courseId=COURSE-1")
                .body_includes("uid=39370562")
                .body_includes("bookId=BOOK-1")
                .body_includes("taskId=CONTENT-1");
            then.status(200).json_body(ok(json!({
                "paperInfo": { "paperData": "<paper/>" },
                "userAnswer": { "score": 8 }
            })));
        });

        let client = client(&server);
        let detail = answer_detail(&client, &params()).await.unwrap();

        assert_eq!(detail.paper_data(), Some("<paper/>"));
        assert_eq!(detail.score(), Some(8.0));
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn v2_param_error_falls_back_to_v1_get() {
        let server = MockServer::start();
        let v2 = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/task/answer/detail/v2");
            then.status(200).json_body(json!({
                "result": { "code": -1, "msg": "param_missing" }
            }));
        });
        let v1 = server.mock(|when, then| {
            when.method(GET)
                .path("/client/student/task/answer/detail")
                .query_param("courseId", "COURSE-1")
                .query_param("uid", "39370562")
                .query_param("bookId", "BOOK-1")
                .query_param("taskId", "CONTENT-1");
            then.status(200).json_body(ok(json!({
                "paperInfo": { "paperData": "<v1/>" },
                "userAnswer": { "markData": "marks" }
            })));
        });

        let client = client(&server);
        let detail = answer_detail(&client, &params()).await.unwrap();

        assert_eq!(detail.paper_data(), Some("<v1/>"));
        assert_eq!(detail.mark_data(), Some("marks"));
        v2.assert_calls(1);
        v1.assert_calls(1);
    }

    #[tokio::test]
    async fn v2_404_falls_back_to_v1() {
        let server = MockServer::start();
        let v2 = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/task/answer/detail/v2");
            then.status(404).body("not found");
        });
        let v1 = server.mock(|when, then| {
            when.method(GET).path("/client/student/task/answer/detail");
            then.status(200)
                .json_body(ok(json!({ "paperInfo": {}, "userAnswer": {} })));
        });

        let client = client(&server);
        answer_detail(&client, &params()).await.unwrap();
        v2.assert_calls(1);
        v1.assert_calls(1);
    }

    #[tokio::test]
    async fn v2_405_falls_back_to_v1() {
        let server = MockServer::start();
        let v2 = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/task/answer/detail/v2");
            then.status(405).body("method not allowed");
        });
        let v1 = server.mock(|when, then| {
            when.method(GET).path("/client/student/task/answer/detail");
            then.status(200)
                .json_body(ok(json!({ "paperInfo": {}, "userAnswer": {} })));
        });

        let client = client(&server);
        answer_detail(&client, &params()).await.unwrap();
        v2.assert_calls(1);
        v1.assert_calls(1);
    }

    #[tokio::test]
    async fn paper_detail_returns_xml_string() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/client/paper/detail")
                .query_param("paperId", "PAPER-1");
            then.status(200).json_body(ok(json!({
                "paperData": "<elements><element id=\"Q1\"/></elements>"
            })));
        });

        let client = client(&server);
        let detail = paper_detail(&client, "PAPER-1").await.unwrap();
        assert_eq!(detail.as_str(), "<elements><element id=\"Q1\"/></elements>");
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn v2_auth_error_does_not_fallback() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/student/task/answer/detail/v2");
            then.status(200)
                .json_body(json!({ "result": { "code": -21, "msg": "no login" } }));
        });

        let client = client(&server);
        let error = answer_detail(&client, &params()).await.unwrap_err();
        assert!(!error.to_string().is_empty());
        mock.assert_calls(1);
    }
}
