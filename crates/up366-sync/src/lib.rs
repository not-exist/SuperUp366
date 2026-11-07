//! Orchestration layer. Skeleton only; the run loop lands in a later step.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SyncError>;
