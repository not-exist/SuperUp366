//! Persistence layer.
//!
//! The schema and migrations land in the storage step; this crate currently
//! exposes only the error type so the workspace links. Kept SQL-free for now so
//! the skeleton builds without pulling in the database driver.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("record not found")]
    NotFound,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
