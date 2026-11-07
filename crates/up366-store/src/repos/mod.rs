//! Repositories. Each one is a thin, typed wrapper over one area of the schema.

pub mod accounts;
pub mod cache;
pub mod settings;

pub use accounts::{AccountRecord, AccountsRepo, NewAccount};
pub use cache::{CacheRepo, CourseRecord, JobRecord, JobStatusRecord};
pub use settings::SettingsRepo;
