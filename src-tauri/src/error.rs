//! Error type exposed to the React layer.
//!
//! Tauri rejects commands with whatever value we serialize, so the error shape
//! doubles as the wire contract. It must stay stable and free of sensitive
//! material: no TGT, no cookies, no password ever appears in a message.

use serde::Serialize;
use up366_core::DomainError;
use up366_protocol::ProtocolError;

/// Anything that can fail with a user-visible message.
pub type CommandResult<T> = Result<T, AppError>;

/// Failure returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    /// Stable machine-readable code, taken from [`DomainError::code`].
    pub code: &'static str,
    /// Human-readable message, safe to render.
    pub message: String,
    /// Whether offering "try again" makes sense.
    pub retryable: bool,
}

impl AppError {
    /// Build an error from an explicit code.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    /// The feature exists upstream but this client does not implement it yet.
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self {
            code: "unsupported",
            message: what.into(),
            retryable: false,
        }
    }

    /// No stored session for the requested account; the user must log in.
    pub fn auth_required() -> Self {
        Self {
            code: "session_expired",
            message: "会话已失效，请重新登录".to_owned(),
            retryable: false,
        }
    }

    /// Whether a retry button should be offered.
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl From<DomainError> for AppError {
    fn from(error: DomainError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
            retryable: error.is_retryable(),
        }
    }
}

impl From<ProtocolError> for AppError {
    fn from(error: ProtocolError) -> Self {
        // The protocol layer already knows the domain mapping; reuse it so a
        // single place decides what a code means.
        Self::from(error.into_domain())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_errors_map_to_stable_codes() {
        let dispatched: Vec<(&str, DomainError)> = vec![
            ("session_expired", DomainError::SessionExpired),
            ("duplicate_submission", DomainError::Duplicate),
            ("network_error", DomainError::Network("timeout".into())),
            (
                "protocol_drift",
                DomainError::ProtocolDrift("new field".into()),
            ),
            ("invalid_input", DomainError::Invalid("empty".into())),
            ("unsupported", DomainError::Unsupported("essay".into())),
            ("api_error", DomainError::api(-13, "already")),
        ];
        for (code, error) in dispatched {
            assert_eq!(AppError::from(error).code, code);
        }
    }

    #[test]
    fn only_network_errors_are_retryable() {
        assert!(AppError::from(DomainError::Network("reset".into())).is_retryable());
        assert!(!AppError::from(DomainError::SessionExpired).is_retryable());
        assert!(!AppError::unsupported("readaloud submit").is_retryable());
    }

    #[test]
    fn serializes_into_the_expected_wire_shape() {
        let json = serde_json::to_value(AppError::auth_required()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "session_expired",
                "message": "会话已失效，请重新登录",
                "retryable": false,
            })
        );
    }
}
