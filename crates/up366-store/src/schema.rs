//! Schema ownership.
//!
//! The authoritative schema lives in `migrations/0001_init.sql`. This module
//! only exposes the embedded migrator so every [`crate::Database`] runs exactly
//! the same migrations, whether it is a file database or an in-memory one.

/// Embedded migration set, applied on database open.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Table names, for diagnostics and tests.
pub const TABLES: &[&str] = &[
    "accounts",
    "courses",
    "jobs",
    "job_status",
    "books",
    "answers",
    "audio_assets",
    "submissions",
    "wrongnote_items",
    "wrongnote_clean_runs",
    "wrongnote_clean_items",
    "settings",
];
