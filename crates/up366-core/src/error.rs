//! Domain-level error type.
//!
//! Transport, storage and UI errors are mapped into these variants so the
//! React layer can render one stable error model instead of a dozen upstream
//! shapes.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DomainError>;

#[derive(Debug, Error)]
pub enum DomainError {
    /// No usable session: the user must log in again.
    #[error("session expired or missing")]
    SessionExpired,

    /// Upstream rejected the credentials.
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    /// Upstream returned a non-zero business code.
    #[error("upstream error code {code}: {message}")]
    Api { code: i64, message: String },

    /// The server said "already submitted / attempts exhausted" (`-13`).
    #[error("already submitted or attempts exhausted")]
    Duplicate,

    /// Transport-level failure.
    #[error("network error: {0}")]
    Network(String),

    /// The upstream payload no longer matches what we parse.
    #[error("protocol drift: {0}")]
    ProtocolDrift(String),

    /// Caller supplied invalid input.
    #[error("invalid input: {0}")]
    Invalid(String),

    /// Feature exists upstream but is not supported by this client.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

impl DomainError {
    pub fn api(code: i64, message: impl Into<String>) -> Self {
        Self::Api {
            code,
            message: message.into(),
        }
    }

    /// Whether the UI should offer a retry button for this error.
    ///
    /// Only transient transport problems are retryable; business codes and
    /// validation errors must not be retried blindly.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    /// Stable machine-readable code for the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SessionExpired => "session_expired",
            Self::AuthFailed(_) => "auth_failed",
            Self::Api { .. } => "api_error",
            Self::Duplicate => "duplicate_submission",
            Self::Network(_) => "network_error",
            Self::ProtocolDrift(_) => "protocol_drift",
            Self::Invalid(_) => "invalid_input",
            Self::Unsupported(_) => "unsupported",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_network_errors_are_retryable() {
        assert!(DomainError::Network("timeout".into()).is_retryable());
        assert!(!DomainError::Duplicate.is_retryable());
        assert!(!DomainError::api(-21, "no login").is_retryable());
    }
}
