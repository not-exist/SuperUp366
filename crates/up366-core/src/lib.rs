//! Pure domain models for the up366 desktop client.
//!
//! This crate performs no I/O and knows nothing about HTTP, SQLite, Tauri or
//! React. Everything here is deterministic and unit-testable.

pub mod deadline;
pub mod error;
pub mod ids;
pub mod job;
pub mod status;
pub mod wordpk;

pub use deadline::parse_deadline;
pub use error::{DomainError, Result};
pub use ids::{AccountId, BookId, CourseId, JobId, QuestionId};
pub use job::JobKind;
pub use status::{resolve_status, DetailState, JobState, StatusInputs};
pub use wordpk::{draw as draw_word_pk, RoundRequest};
