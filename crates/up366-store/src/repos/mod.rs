//! Repositories. Each one is a thin, typed wrapper over one area of the schema.

pub mod accounts;
pub mod audio;
pub mod cache;
pub mod settings;

pub use accounts::{AccountRecord, AccountsRepo, NewAccount};
pub use audio::{AudioAssetRecord, AudioRepo, NewAudioAsset};
pub use cache::{CacheRepo, CourseRecord, JobRecord, JobStatusRecord};
pub use settings::SettingsRepo;
