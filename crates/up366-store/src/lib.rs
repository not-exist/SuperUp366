//! Persistence layer.
//!
//! One SQLite database holds cached upstream data, local state and the
//! submission outbox. Credentials never touch the database: session blobs are
//! kept in the OS credential store behind [`secrets::SecretStore`].
//!
//! Open a database with [`Database::open`] (or [`Database::memory`] in tests)
//! and reach the repositories through its accessors:
//!
//! ```no_run
//! # async fn example() -> up366_store::Result<()> {
//! use up366_store::{Database, NewAccount};
//! use up366_core::AccountId;
//!
//! let db = Database::open("up366.db").await?;
//! let account = NewAccount::new(AccountId::from("acc-1"));
//! db.accounts().upsert(&account).await?;
//! # Ok(())
//! # }
//! ```

pub mod db;
pub mod repos;
pub mod schema;
pub mod secrets;

pub use db::{now_millis, Database};
pub use repos::accounts::{AccountRecord, AccountsRepo, NewAccount};
pub use repos::audio::{AudioAssetRecord, AudioRepo, NewAudioAsset};
pub use repos::cache::{CacheRepo, CourseRecord, JobRecord, JobStatusRecord};
pub use repos::settings::SettingsRepo;
pub use secrets::{KeyringStore, MemoryStore, SecretStore, KEYRING_SERVICE};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("record not found")]
    NotFound,
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("credential store error: {0}")]
    Secret(#[from] keyring::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;
