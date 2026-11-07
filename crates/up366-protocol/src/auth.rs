//! Username/password login.
//!
//! The wire protocol is:
//!
//! ```text
//! password = md5(md5(plain) + salt)
//! ut       = md5(ts)[..10] + md5(username + password + "loginEncryptionParameter" + md5(ts)) + md5(ts)[10..]
//! POST sso /v2/tickets-v3  { username, password, ut }
//! success code: -26, data.tgt is the TGT, data.uType the account type
//! ```
//!
//! Two password salts exist in the recon archive. The reference PC client and
//! the live probes use [`PASSWORD_SALT_AT`] (`@`), so that is the default; the
//! colon variant is exposed for the field verification step only.

use serde_json::{json, Value};

use crate::client::Client;
use crate::codec;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};
use crate::session::Session;
use crate::signature::{login_password_hash, login_ut, PASSWORD_SALT_AT, PASSWORD_SALT_COLON};

/// Which password salt to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordSalt {
    /// `fa&s*l%$k!fq$k!ld@fjlk` — used by the PC client and live probes.
    #[default]
    AtSign,
    /// `fa&s*l%$k!fq$k!ld:fjlk` — present in some scripts; unverified.
    Colon,
}

impl PasswordSalt {
    pub fn value(self) -> &'static str {
        match self {
            Self::AtSign => PASSWORD_SALT_AT,
            Self::Colon => PASSWORD_SALT_COLON,
        }
    }

    /// Client-compatible password hash for this salt.
    pub fn hash(self, password: &str) -> String {
        login_password_hash(password, self.value())
    }
}

/// Log in and return a session. Uses the default (`@`) salt.
pub async fn login(client: &Client, username: &str, password: &str) -> Result<Session> {
    login_with_salt(client, username, password, PasswordSalt::default()).await
}

/// Log in with an explicitly selected password salt.
///
/// The client is updated in place (TGT, username, cookies), so callers should
/// persist the returned [`Session`] and reuse the same client afterwards.
pub async fn login_with_salt(
    client: &Client,
    username: &str,
    password: &str,
    salt: PasswordSalt,
) -> Result<Session> {
    let endpoint =
        get("login").ok_or_else(|| ProtocolError::Decode("missing endpoint: login".into()))?;

    let password_hash = salt.hash(password);
    let signature = login_ut(username, &password_hash, client.now_ms());
    let fields = [
        ("username", json!(username)),
        ("password", json!(password_hash)),
        ("ut", json!(signature)),
    ];

    // Anonymous: the login request carries no TGT (`u3t` is empty).
    let data = client
        .call_with_codes(endpoint, &fields, &[-26], true)
        .await
        .map_err(auth_error)?;

    let object = codec::into_object(data, "login").map_err(auth_error)?;
    let tgt = codec::string_field(&object, "tgt")
        .filter(|tgt| !tgt.is_empty())
        .ok_or_else(|| ProtocolError::Auth("login response did not contain data.tgt".into()))?;
    let user_type = object.get("uType").and_then(codec_i64);

    let session = Session {
        username: username.to_owned(),
        tgt,
        client_id: client.client_id().to_owned(),
        user_type,
        cookies: client.export_cookies(),
        expired: false,
    };
    client.apply_session(&session);
    Ok(session)
}

/// Login failures are credential failures, not transport failures.
fn auth_error(error: ProtocolError) -> ProtocolError {
    match error {
        ProtocolError::Api { code, message } => {
            ProtocolError::Auth(format!("login failed (code {code}): {message}"))
        }
        ProtocolError::Http { status, body } => {
            ProtocolError::Auth(format!("login failed (HTTP {status}): {body}"))
        }
        ProtocolError::Decode(message) => ProtocolError::Auth(message),
        other => other,
    }
}

fn codec_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::{login_ut, PASSWORD_SALT_AT};
    use httpmock::prelude::*;

    const CLOCK: u128 = 1_700_000_000_123;

    fn client(server: &MockServer) -> Client {
        Client::builder()
            .client_id("CID-TEST")
            .base_url_override(server.base_url())
            .retry_base_delay(std::time::Duration::ZERO)
            .clock(|| CLOCK)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn login_stores_tgt_cookies_and_user_type() {
        let server = MockServer::start();
        let password_hash = login_password_hash("secret", PASSWORD_SALT_AT);
        let signature = login_ut("alice", &password_hash, CLOCK);
        let expected = format!("username=alice&password={password_hash}&ut={signature}");

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v2/tickets-v3")
                .header("clientid", "CID-TEST")
                .header("u3t", "")
                .body_contains(expected);
            then.status(200)
                .header("set-cookie", "CASTGC=abc; Path=/")
                .json_body(json!({
                    "result": { "code": -26, "msg": "ok" },
                    "data": { "tgt": "TGT-9", "uType": 102 }
                }));
        });

        let client = client(&server);
        let session = login(&client, "alice", "secret").await.unwrap();

        assert_eq!(session.username, "alice");
        assert_eq!(session.tgt, "TGT-9");
        assert_eq!(session.client_id, "CID-TEST");
        assert_eq!(session.user_type, Some(102));
        assert!(!session.is_expired());
        assert_eq!(session.cookie("CASTGC"), Some("abc"));
        assert_eq!(client.tgt(), "TGT-9");
        assert_eq!(client.username(), "alice");

        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn wrong_password_is_an_auth_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v2/tickets-v3");
            then.status(200)
                .json_body(json!({ "result": { "code": -27, "msg": "用户名或密码错误" } }));
        });

        let client = client(&server);
        let error = login(&client, "alice", "wrong").await.unwrap_err();

        assert!(matches!(error, ProtocolError::Auth(_)));
        assert!(error.to_string().contains("-27"));
    }

    #[tokio::test]
    async fn login_without_tgt_is_an_auth_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v2/tickets-v3");
            then.status(200).json_body(json!({
                "result": { "code": -26, "msg": "ok" },
                "data": { "uType": 102 }
            }));
        });

        let client = client(&server);
        let error = login(&client, "alice", "secret").await.unwrap_err();
        assert!(matches!(error, ProtocolError::Auth(_)));
        assert!(error.to_string().contains("tgt"));
    }

    #[tokio::test]
    async fn colon_salt_changes_the_password_hash() {
        let server = MockServer::start();
        let expected_hash = login_password_hash("secret", PASSWORD_SALT_COLON);
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/v2/tickets-v3")
                .body_contains(format!("password={expected_hash}"));
            then.status(200).json_body(json!({
                "result": { "code": -26, "msg": "ok" },
                "data": { "tgt": "TGT-9" }
            }));
        });

        let client = client(&server);
        let session = login_with_salt(&client, "alice", "secret", PasswordSalt::Colon)
            .await
            .unwrap();
        assert_eq!(session.tgt, "TGT-9");
        mock.assert_hits(1);
    }
}
