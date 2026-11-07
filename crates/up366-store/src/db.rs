//! Database handle and connection policy.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::repos::{AccountsRepo, CacheRepo, SettingsRepo};
use crate::Result;

/// Current wall-clock time in Unix epoch milliseconds.
///
/// The whole schema uses this representation: it sorts naturally, needs no
/// timezone handling and keeps SQLite comparisons cheap.
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A SQLite database with the up366 schema applied.
///
/// Cloning is cheap: the underlying [`SqlitePool`] is shared. Repositories are
/// obtained with [`Database::accounts`], [`Database::cache`] and
/// [`Database::settings`].
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open (or create) a file-backed database and run migrations.
    ///
    /// WAL keeps readers from blocking the writer, and the busy timeout makes
    /// concurrent repository calls wait instead of failing with `SQLITE_BUSY`.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        Self::from_pool(pool).await
    }

    /// Open a private in-memory database, for tests.
    ///
    /// The pool is pinned to one connection: separate SQLite connections to
    /// `:memory:` would each see a different, empty database.
    pub async fn memory() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .connect_with(options)
            .await?;

        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Result<Self> {
        crate::schema::MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// The shared connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Account records.
    pub fn accounts(&self) -> AccountsRepo {
        AccountsRepo::new(self.pool.clone())
    }

    /// Cached courses, jobs and job statuses.
    pub fn cache(&self) -> CacheRepo {
        CacheRepo::new(self.pool.clone())
    }

    /// Application settings.
    pub fn settings(&self) -> SettingsRepo {
        SettingsRepo::new(self.pool.clone())
    }

    /// Close the pool, waiting for in-flight queries.
    pub async fn close(self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_create_every_table() {
        let db = Database::memory().await.unwrap();
        for table in crate::schema::TABLES {
            let name: String = sqlx::query_scalar(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(db.pool())
            .await
            .unwrap();
            assert_eq!(&name, table);
        }
    }

    #[tokio::test]
    async fn opening_the_same_file_twice_keeps_data() {
        // Unique per run: tests in one binary share the process id, and a
        // stale directory from a previous failure must not collide.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("up366-store-{stamp}"));
        let path = dir.join("nested").join("db.sqlite");
        let _ = std::fs::remove_dir_all(&dir);

        let db = Database::open(&path).await.unwrap();
        db.settings().set("theme", "dark").await.unwrap();
        db.close().await;

        let db = Database::open(&path).await.unwrap();
        assert_eq!(
            db.settings().get("theme").await.unwrap().as_deref(),
            Some("dark")
        );
        db.close().await;
        // Best effort: SQLite may still hold the file briefly on Windows.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
