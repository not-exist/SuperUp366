//! Session context endpoints: the logged-in user's numeric id and server time.
//!
//! Both are required by the wrong-note cleanup submission:
//!
//! - `uid` is a numeric student id. The local account id is the lower-cased
//!   username, so it is not usable as `uid`.
//! - `startTime` / `endTime` are server milliseconds, not the local clock.
//!
//! The endpoints already exist in the table; this module is their first caller.

use serde::Deserialize;
use serde_json::Value;

use crate::client::Client;
use crate::codec;
use crate::dto::flex;
use crate::error::{ProtocolError, Result};

use super::endpoint;

/// The current user, as `user-api /client/user/currentuser` describes them.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUser {
    /// The numeric student id used by the wrong-note write endpoints.
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub uid: String,
    #[serde(default, deserialize_with = "flex::string_or_empty")]
    pub realname: String,
    /// School id, spelt `organId` upstream.
    #[serde(default, rename = "organId", deserialize_with = "flex::opt_string")]
    pub school_id: Option<String>,
    /// Stage marker (`3` = high school), spelt `schoolType` upstream.
    #[serde(default, rename = "schoolType", deserialize_with = "flex::opt_string")]
    pub school_type: Option<String>,
}

/// Fetch the logged-in user's profile.
pub async fn current_user(client: &Client) -> Result<CurrentUser> {
    let endpoint = endpoint("current_user")?;
    let data = client.call(endpoint, &[]).await?;
    let object = codec::into_object(data, endpoint.name)?;
    let user: CurrentUser = serde_json::from_value(Value::Object(object))
        .map_err(|error| ProtocolError::Decode(format!("current_user: {error}")))?;
    if user.uid.trim().is_empty() {
        return Err(ProtocolError::Decode(
            "current_user: missing uid".to_owned(),
        ));
    }
    Ok(user)
}

/// The server's current time in milliseconds.
pub async fn server_time(client: &Client) -> Result<i64> {
    let endpoint = endpoint("current_time")?;
    let data = client.call(endpoint, &[]).await?;
    let object = codec::into_object(data, endpoint.name)?;
    object
        .get("currentTime")
        .and_then(millis)
        .ok_or_else(|| ProtocolError::Decode("current_time: missing currentTime".to_owned()))
}

/// A JSON number or numeric string as milliseconds.
fn millis(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn current_user_reads_the_numeric_uid() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/user/currentuser");
            then.status(200).json_body(ok(json!({
                "uid": 39370562,
                "realname": "张三",
                "organId": 1001
            })));
        });

        let user = current_user(&client(&server)).await.unwrap();
        assert_eq!(user.uid, "39370562");
        assert_eq!(user.realname, "张三");
        assert_eq!(user.school_id.as_deref(), Some("1001"));
        mock.assert_calls(1);
    }

    #[tokio::test]
    async fn current_user_without_a_uid_is_a_decode_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/client/user/currentuser");
            then.status(200)
                .json_body(ok(json!({ "realname": "张三" })));
        });

        assert!(matches!(
            current_user(&client(&server)).await,
            Err(ProtocolError::Decode(_))
        ));
    }

    #[tokio::test]
    async fn server_time_reads_current_time_in_milliseconds() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/front/current-time");
            then.status(200)
                .json_body(ok(json!({ "currentTime": 1_762_000_000_000i64 })));
        });

        assert_eq!(
            server_time(&client(&server)).await.unwrap(),
            1_762_000_000_000
        );
        mock.assert_calls(1);
    }
}
