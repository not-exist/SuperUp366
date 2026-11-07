//! Protocol-layer error type and mapping to the core domain error.

use thiserror::Error;
use up366_core::DomainError;

/// Result alias for the protocol layer.
pub type Result<T> = std::result::Result<T, ProtocolError>;

/// Error produced by the transport / protocol layer.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// The server answered with a non-zero business code.
    #[error("api error {code}: {message}")]
    Api { code: i64, message: String },

    /// The HTTP response had an unexpected status.
    #[error("http error {status}: {body}")]
    Http { status: u16, body: String },

    /// Underlying transport (connection / TLS / timeout) failure.
    #[error("transport error: {0}")]
    Transport(String),

    /// The response body could not be decoded as expected.
    #[error("decode error: {0}")]
    Decode(String),

    /// The caller is not (or no longer) authenticated.
    #[error("authentication error: {0}")]
    Auth(String),
}

impl ProtocolError {
    /// Map this protocol error onto the pure-domain error type.
    ///
    /// Notable business-code mappings (matching the reference client):
    /// - `-13` -> `DomainError::Duplicate`
    /// - `-21` / `-22` -> `DomainError::SessionExpired`
    pub fn into_domain(self) -> DomainError {
        match self {
            ProtocolError::Api { code, message } => match code {
                -13 => DomainError::Duplicate,
                -21 | -22 => DomainError::SessionExpired,
                -23 | -26 | -27 => DomainError::AuthFailed(message),
                _ => DomainError::api(code, message),
            },
            ProtocolError::Http {
                status: 401 | 403,
                body,
            } => DomainError::AuthFailed(body),
            ProtocolError::Http { status, body } => {
                DomainError::Network(format!("HTTP {status}: {body}"))
            }
            ProtocolError::Transport(message) => DomainError::Network(message),
            ProtocolError::Decode(message) => DomainError::ProtocolDrift(message),
            ProtocolError::Auth(message) => DomainError::AuthFailed(message),
        }
    }

    /// Whether retrying the same request may succeed.
    ///
    /// Only transient network / 5xx / 429 failures qualify. Business codes
    /// (for example `-13`) are permanent and must never be retried.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(_) => true,
            Self::Http { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }

    /// Whether the business code signals a missing or rejected session.
    pub fn is_session_expired(&self) -> bool {
        matches!(
            self,
            Self::Api {
                code: -21 | -22,
                ..
            }
        )
    }
}

impl From<reqwest::Error> for ProtocolError {
    fn from(err: reqwest::Error) -> Self {
        if let Some(status) = err.status() {
            return ProtocolError::Http {
                status: status.as_u16(),
                body: err.to_string(),
            };
        }
        ProtocolError::Transport(err.to_string())
    }
}

impl From<serde_json::Error> for ProtocolError {
    fn from(err: serde_json::Error) -> Self {
        ProtocolError::Decode(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_business_codes_to_domain_errors() {
        assert!(matches!(
            ProtocolError::Api {
                code: -13,
                message: String::new()
            }
            .into_domain(),
            DomainError::Duplicate
        ));
        for code in [-21, -22] {
            assert!(matches!(
                ProtocolError::Api {
                    code,
                    message: String::new()
                }
                .into_domain(),
                DomainError::SessionExpired
            ));
        }
        assert!(matches!(
            ProtocolError::Api {
                code: -27,
                message: "bad".into()
            }
            .into_domain(),
            DomainError::AuthFailed(_)
        ));
        assert!(matches!(
            ProtocolError::Api {
                code: -4,
                message: "no auth".into()
            }
            .into_domain(),
            DomainError::Api { code: -4, .. }
        ));
    }

    #[test]
    fn only_transient_failures_are_retryable() {
        assert!(ProtocolError::Transport("timeout".into()).is_retryable());
        assert!(ProtocolError::Http {
            status: 503,
            body: String::new()
        }
        .is_retryable());
        assert!(ProtocolError::Http {
            status: 429,
            body: String::new()
        }
        .is_retryable());
        assert!(!ProtocolError::Http {
            status: 404,
            body: String::new()
        }
        .is_retryable());
        assert!(!ProtocolError::Api {
            code: -13,
            message: String::new()
        }
        .is_retryable());
        assert!(!ProtocolError::Decode("bad".into()).is_retryable());
    }

    #[test]
    fn maps_transport_and_decode() {
        assert!(matches!(
            ProtocolError::Transport("dns".into()).into_domain(),
            DomainError::Network(_)
        ));
        assert!(matches!(
            ProtocolError::Decode("shape".into()).into_domain(),
            DomainError::ProtocolDrift(_)
        ));
    }
}
