//! Persisted login session.
//!
//! `Session` is what the store layer writes into the OS credential vault: the
//! TGT, the stable device id, the account identity and the login cookies. It
//! deliberately contains no plaintext password and no password hash that can
//! be replayed on its own.
//!
//! Cookies are exported as ordered `(name, value)` pairs. The original domain
//! is not preserved: every up366 host lives under `up366.cn`, so re-importing
//! them as domain cookies for `up366.cn` is both sufficient and simpler than
//! persisting a full RFC 6265 store.

use serde::{Deserialize, Serialize};

/// A restorable authenticated session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Account name used for this session.
    pub username: String,
    /// Ticket-granting ticket, sent as the `u3t` header.
    pub tgt: String,
    /// Stable device id, sent as the `clientId` header.
    pub client_id: String,
    /// `data.uType` from the login response; students are usually `102`.
    #[serde(default)]
    pub user_type: Option<i64>,
    /// Login cookies captured from the `Set-Cookie` headers.
    #[serde(default)]
    pub cookies: Vec<(String, String)>,
    /// Set after upstream rejected the session; the UI must log in again.
    #[serde(default)]
    pub expired: bool,
}

impl Session {
    /// Build a session. Cookies start empty; import them separately.
    pub fn new(
        username: impl Into<String>,
        tgt: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        Self {
            username: username.into(),
            tgt: tgt.into(),
            client_id: client_id.into(),
            user_type: None,
            cookies: Vec::new(),
            expired: false,
        }
    }

    /// Whether the session has been marked unusable.
    ///
    /// This is an explicit flag, not a cryptographic check. A session can be
    /// revoked upstream without us knowing; only a rejected request flips it.
    pub fn is_expired(&self) -> bool {
        self.expired
    }

    /// Mark the session unusable so the next caller prompts for a password.
    pub fn mark_expired(&mut self) {
        self.expired = true;
    }

    /// Whether there is any credential at all (TGT or cookies).
    ///
    /// Either a login cookie or a one-time service ticket is sufficient for
    /// business hosts; `u3t` alone is not.
    pub fn has_credentials(&self) -> bool {
        !self.tgt.is_empty() || !self.cookies.is_empty()
    }

    /// Look up one exported cookie by name.
    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip_keeps_credentials_and_expiry() {
        let mut session = Session::new("alice", "TGT-1", "CID-1");
        session.user_type = Some(102);
        session.cookies = vec![("CASTGC".into(), "abc".into())];
        session.mark_expired();

        let encoded = serde_json::to_string(&session).unwrap();
        let decoded: Session = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, session);
        assert!(decoded.is_expired());
        assert_eq!(decoded.cookie("CASTGC"), Some("abc"));
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let decoded: Session =
            serde_json::from_str(r#"{"username":"alice","tgt":"TGT-1","client_id":"CID-1"}"#)
                .unwrap();
        assert!(!decoded.is_expired());
        assert!(decoded.cookies.is_empty());
        assert!(decoded.has_credentials());
    }
}
