//! Application settings.
//!
//! Settings are global (not per account) and store opaque strings; callers
//! decide whether a value is JSON or a plain scalar.

use sqlx::{Row, SqlitePool};

use crate::db::now_millis;
use crate::Result;

#[derive(Clone)]
pub struct SettingsRepo {
    pool: SqlitePool,
}

impl SettingsRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace one setting.
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET
                 value      = excluded.value,
                 updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read one setting.
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        row.map(|row| row.try_get::<String, _>("value"))
            .transpose()
            .map_err(Into::into)
    }

    /// Delete one setting. Returns whether it existed.
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// All settings, ordered by key.
    pub async fn list(&self) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query("SELECT key, value FROM settings ORDER BY key")
            .fetch_all(&self.pool)
            .await?;
        rows.iter()
            .map(|row| Ok((row.try_get("key")?, row.try_get("value")?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn set_get_delete_round_trip() {
        let db = Database::memory().await.unwrap();
        let repo = db.settings();

        assert!(repo.get("theme").await.unwrap().is_none());
        repo.set("theme", "dark").await.unwrap();
        repo.set("locale", "zh-CN").await.unwrap();
        repo.set("theme", "light").await.unwrap();

        assert_eq!(repo.get("theme").await.unwrap().as_deref(), Some("light"));
        assert_eq!(
            repo.list().await.unwrap(),
            vec![
                ("locale".to_owned(), "zh-CN".to_owned()),
                ("theme".to_owned(), "light".to_owned()),
            ]
        );

        assert!(repo.delete("theme").await.unwrap());
        assert!(!repo.delete("theme").await.unwrap());
        assert!(repo.get("theme").await.unwrap().is_none());
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }
}
