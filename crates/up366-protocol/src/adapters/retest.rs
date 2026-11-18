//! Retest ("重新测试"): probe whether a submitted homework may be redone and
//! reset it before the ordinary submit.
//!
//! The official report page reads `onlinemarking/alldata`, takes
//! `data.testInfo.parameters` (a JSON string) and shows the button only when
//! `reTest == 1`. Pressing it posts `homework/test/redo` with
//! `{testId, returnType: "1"}`; code 0 means the attempt was reset, 1 means the
//! practice is closed, anything else that the paper does not support it.
//!
//! `reTest` is a template flag, not a permission: it also appears on jobs that
//! were never submitted. Only the redo response decides.

use serde_json::{json, Value};

use crate::client::Client;
use crate::error::{ProtocolError, Result};

/// The outcome of probing one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetestSupport {
    pub supported: bool,
    /// Why it is not supported; empty when `supported`.
    pub reason: String,
}

/// The server's answer to a redo request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedoOutcome {
    /// The attempt was reset; submit again through the ordinary route.
    Accepted,
    /// The practice is already closed.
    Closed { message: String },
    /// This paper does not support retesting.
    Unsupported { message: String },
}

/// Ask the server whether the job's paper allows a retest.
///
/// The report page calls the same endpoint with the job's `contentId` and
/// `uid`.
pub async fn probe_retest(client: &Client, test_id: &str, uid: i64) -> Result<RetestSupport> {
    let endpoint = super::endpoint("onlinemarking_alldata")?;
    let data = match client
        .call(endpoint, &[("testId", json!(test_id)), ("uid", json!(uid))])
        .await
    {
        Ok(data) => data,
        // The engine answers "测验不存在!" (1) for a paper it never hosted;
        // `homework/test/info` answers -3/no_data for the same job. Both mean
        // "this job has no online-test record", not "the probe failed".
        Err(ProtocolError::Api { code: 1, .. } | ProtocolError::Api { code: -3, .. }) => {
            return Ok(RetestSupport {
                supported: false,
                reason: "该作业没有在线测验记录，不支持重做".to_owned(),
            })
        }
        Err(error) => return Err(error),
    };
    let data = data.unwrap_or(Value::Null);
    let re_test = data.pointer("/testInfo/parameters").and_then(re_test_value);
    Ok(match re_test {
        Some(1) => RetestSupport {
            supported: true,
            reason: String::new(),
        },
        Some(_) => RetestSupport {
            supported: false,
            reason: "该作业未开启重做".to_owned(),
        },
        None => RetestSupport {
            supported: false,
            reason: "服务端未返回重做配置".to_owned(),
        },
    })
}

/// Reset a submitted attempt so the ordinary submit is recorded again.
///
/// A business code of 1 is "practice closed"; any other non-negative code means
/// the paper does not support retesting. Session and transport errors are
/// returned as errors: they say nothing about the paper.
pub async fn redo_homework(client: &Client, test_id: &str) -> Result<RedoOutcome> {
    let endpoint = super::endpoint("homework_test_redo")?;
    match client
        .call(
            endpoint,
            &[("testId", json!(test_id)), ("returnType", json!("1"))],
        )
        .await
    {
        Ok(_) => Ok(RedoOutcome::Accepted),
        Err(ProtocolError::Api { code: 1, message }) => Ok(RedoOutcome::Closed { message }),
        Err(ProtocolError::Api { code, message }) if code >= 0 => Ok(RedoOutcome::Unsupported {
            message: if message.is_empty() {
                format!("服务端返回 {code}")
            } else {
                message
            },
        }),
        Err(other) => Err(other),
    }
}

/// Read `reTest` from `testInfo.parameters`.
///
/// The page `eval`s the string, so it may be JSON or a JavaScript object
/// literal. Prefer `serde_json`, then fall back to scanning the key.
fn re_test_value(value: &Value) -> Option<i64> {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|parsed| re_test_value(&parsed))
            .or_else(|| scan_re_test(text)),
        Value::Object(object) => object.get("reTest").and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        }),
        Value::Number(number) => number.as_i64(),
        _ => None,
    }
}

/// Tolerant fallback for a non-JSON parameters string.
fn scan_re_test(text: &str) -> Option<i64> {
    let start = text.find("reTest")? + "reTest".len();
    let rest = text[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.trim_start_matches(['"', '\'']);
    let digits: String = rest
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    fn parameters(value: Value) -> Value {
        json!({ "testInfo": { "parameters": value } })
    }

    #[tokio::test]
    async fn a_re_test_flag_of_one_is_supported() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/onlinemarking/alldata")
                .body_includes("testId=CONTENT-1")
                .body_includes("uid=39370562");
            then.status(200).json_body(ok(parameters(json!(
                r#"{"paperType":5,"reTest":1,"pastExercise":1}"#
            ))));
        });

        let support = probe_retest(&client(&server), "CONTENT-1", 39_370_562)
            .await
            .unwrap();
        assert!(support.supported);
        assert!(support.reason.is_empty());
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn a_zero_flag_is_not_supported() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/onlinemarking/alldata");
            then.status(200)
                .json_body(ok(parameters(json!({"reTest": 0}))));
        });

        let support = probe_retest(&client(&server), "CONTENT-1", 1)
            .await
            .unwrap();
        assert!(!support.supported);
        assert_eq!(support.reason, "该作业未开启重做");
    }

    #[tokio::test]
    async fn a_missing_parameters_document_is_not_supported() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/onlinemarking/alldata");
            then.status(200).json_body(ok(json!({ "testInfo": {} })));
        });

        let support = probe_retest(&client(&server), "CONTENT-1", 1)
            .await
            .unwrap();
        assert!(!support.supported);
        assert_eq!(support.reason, "服务端未返回重做配置");
    }

    #[tokio::test]
    async fn a_missing_test_record_is_not_supported() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/onlinemarking/alldata");
            then.status(200)
                .json_body(json!({ "result": { "code": 1, "msg": "测验不存在!" } }));
        });

        let support = probe_retest(&client(&server), "CONTENT-1", 1)
            .await
            .unwrap();
        assert!(!support.supported);
        assert_eq!(support.reason, "该作业没有在线测验记录，不支持重做");
    }

    #[tokio::test]
    async fn a_javascript_literal_is_scanned() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/onlinemarking/alldata");
            then.status(200).json_body(ok(parameters(json!(
                "{paperType:5,reTest:1,pastExercise:1}"
            ))));
        });

        let support = probe_retest(&client(&server), "CONTENT-1", 1)
            .await
            .unwrap();
        assert!(support.supported);
    }

    #[tokio::test]
    async fn redo_accepts_and_posts_the_page_payload() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/homework/test/redo")
                .body_includes("testId=CONTENT-1")
                .body_includes("returnType=1");
            then.status(200).json_body(ok(json!(null)));
        });

        let outcome = redo_homework(&client(&server), "CONTENT-1").await.unwrap();
        assert_eq!(outcome, RedoOutcome::Accepted);
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn redo_reports_a_closed_practice() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/homework/test/redo");
            then.status(200).json_body(json!({
                "result": { "code": 1, "msg": "练习已经结束，不可以再重新测试" }
            }));
        });

        let outcome = redo_homework(&client(&server), "CONTENT-1").await.unwrap();
        assert_eq!(
            outcome,
            RedoOutcome::Closed {
                message: "练习已经结束，不可以再重新测试".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn redo_reports_an_unsupported_paper() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/homework/test/redo");
            then.status(200)
                .json_body(json!({ "result": { "code": 2, "msg": "该练习不支持重做" } }));
        });

        let outcome = redo_homework(&client(&server), "CONTENT-1").await.unwrap();
        assert_eq!(
            outcome,
            RedoOutcome::Unsupported {
                message: "该练习不支持重做".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn redo_propagates_a_session_failure() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/homework/test/redo");
            then.status(200)
                .json_body(json!({ "result": { "code": -23, "msg": "找不到tgt" } }));
        });

        let error = redo_homework(&client(&server), "CONTENT-1")
            .await
            .unwrap_err();
        assert!(matches!(error, ProtocolError::Api { code: -23, .. }));
    }
}
