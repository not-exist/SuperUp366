//! Media pipeline. Skeleton only; probing/transcoding land in a later step.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, MediaError>;
