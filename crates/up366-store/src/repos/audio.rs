//! Local recordings and their OSS destinations.
//!
//! One row per `(job, question)`: uploading again replaces the previous file.
//! The row keeps both the object name (what the upload used) and the resolved
//! URL (what the score payload references), because the bucket host is only
//! known from the STS response.

use sqlx::{Row, SqlitePool};
use up366_core::AccountId;

use crate::Result;

/// One uploaded recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioAssetRecord {
    pub asset_id: String,
    pub job_id: String,
    pub question_id: String,
    pub local_path: String,
    pub object_name: Option<String>,
    pub object_url: Option<String>,
    pub size: i64,
    pub sha256: String,
    pub duration_ms: Option<i64>,
    pub created_at: i64,
}

/// Fields needed to store one recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAudioAsset {
    pub asset_id: String,
    pub job_id: String,
    pub question_id: String,
    pub local_path: String,
    pub object_name: String,
    pub object_url: String,
    pub size: i64,
    pub sha256: String,
    pub duration_ms: Option<i64>,
}

#[derive(Clone)]
pub struct AudioRepo {
    pool: SqlitePool,
}

impl AudioRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace the recording for one slot.
    pub async fn upsert(&self, account_id: &AccountId, asset: &NewAudioAsset) -> Result<()> {
        sqlx::query(
            "INSERT INTO audio_assets (
                 account_id, asset_id, job_id, question_id, local_path, object_name,
                 object_url, size, sha256, duration_ms, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(account_id, asset_id) DO UPDATE SET
                 local_path  = excluded.local_path,
                 object_name = excluded.object_name,
                 object_url  = excluded.object_url,
                 size        = excluded.size,
                 sha256      = excluded.sha256,
                 duration_ms = excluded.duration_ms,
                 created_at  = excluded.created_at",
        )
        .bind(account_id.as_str())
        .bind(&asset.asset_id)
        .bind(&asset.job_id)
        .bind(&asset.question_id)
        .bind(&asset.local_path)
        .bind(&asset.object_name)
        .bind(&asset.object_url)
        .bind(asset.size)
        .bind(&asset.sha256)
        .bind(asset.duration_ms)
        .bind(crate::now_millis())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// All recordings stored for one job, in upload order.
    pub async fn for_job(
        &self,
        account_id: &AccountId,
        job_id: &str,
    ) -> Result<Vec<AudioAssetRecord>> {
        let rows = sqlx::query(
            "SELECT asset_id, job_id, question_id, local_path, object_name, object_url,
                    size, sha256, duration_ms, created_at
             FROM audio_assets
             WHERE account_id = ? AND job_id = ?
             ORDER BY created_at, question_id",
        )
        .bind(account_id.as_str())
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(map_asset).collect()
    }

    /// Drop the recordings of one job.
    pub async fn delete_for_job(&self, account_id: &AccountId, job_id: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM audio_assets WHERE account_id = ? AND job_id = ?")
            .bind(account_id.as_str())
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

fn map_asset(row: &sqlx::sqlite::SqliteRow) -> Result<AudioAssetRecord> {
    Ok(AudioAssetRecord {
        asset_id: row.try_get("asset_id")?,
        job_id: row.try_get("job_id")?,
        question_id: row.try_get("question_id")?,
        local_path: row.try_get("local_path")?,
        object_name: row.try_get("object_name")?,
        object_url: row.try_get("object_url")?,
        size: row.try_get("size")?,
        sha256: row.try_get("sha256")?,
        duration_ms: row.try_get("duration_ms")?,
        created_at: row.try_get("created_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, NewAccount};

    async fn database() -> (Database, AccountId) {
        let db = Database::memory().await.unwrap();
        let account = AccountId::from("acc-1");
        db.accounts()
            .upsert(&NewAccount::new(account.clone()))
            .await
            .unwrap();
        (db, account)
    }

    fn asset(question: &str, url: &str) -> NewAudioAsset {
        NewAudioAsset {
            asset_id: format!("JOB:{question}"),
            job_id: "JOB".to_owned(),
            question_id: question.to_owned(),
            local_path: "C:/tmp/a.mp3".to_owned(),
            object_name: "storage/102/a.mp3".to_owned(),
            object_url: url.to_owned(),
            size: 12,
            sha256: "abc".to_owned(),
            duration_ms: None,
        }
    }

    #[tokio::test]
    async fn uploading_again_replaces_the_slot() {
        let (db, account) = database().await;
        db.audio()
            .upsert(&account, &asset("Q1", "https://oss/1.mp3"))
            .await
            .unwrap();
        db.audio()
            .upsert(&account, &asset("Q1", "https://oss/2.mp3"))
            .await
            .unwrap();

        let rows = db.audio().for_job(&account, "JOB").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].object_url.as_deref(), Some("https://oss/2.mp3"));
    }

    #[tokio::test]
    async fn rows_are_scoped_to_the_job() {
        let (db, account) = database().await;
        db.audio()
            .upsert(&account, &asset("Q1", "https://oss/1.mp3"))
            .await
            .unwrap();
        assert!(db
            .audio()
            .for_job(&account, "OTHER")
            .await
            .unwrap()
            .is_empty());
        assert_eq!(db.audio().delete_for_job(&account, "JOB").await.unwrap(), 1);
    }
}
