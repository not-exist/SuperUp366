//! Account records.
//!
//! Accounts are local identities. The upstream session itself is *not* stored
//! here; see [`crate::secrets`].

use sqlx::{Row, SqlitePool};
use up366_core::AccountId;

use crate::db::now_millis;
use crate::{Result, StoreError};

/// Input for [`AccountsRepo::upsert`]. Fields that upstream may omit are
/// optional; the local `id` is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccount {
    pub id: AccountId,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub uid: Option<String>,
    pub school_id: Option<String>,
    pub user_type: Option<i64>,
    pub client_id: Option<String>,
    pub status: String,
    pub last_login_at: Option<i64>,
}

impl NewAccount {
    /// A new account with only an id; everything else can be filled in later
    /// by an upsert once the login response is parsed.
    pub fn new(id: impl Into<AccountId>) -> Self {
        Self {
            id: id.into(),
            username: None,
            display_name: None,
            uid: None,
            school_id: None,
            user_type: None,
            client_id: None,
            status: "active".to_owned(),
            last_login_at: None,
        }
    }
}

/// A stored account row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRecord {
    pub id: AccountId,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub uid: Option<String>,
    pub school_id: Option<String>,
    pub user_type: Option<i64>,
    pub client_id: Option<String>,
    pub status: String,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone)]
pub struct AccountsRepo {
    pool: SqlitePool,
}

impl AccountsRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update an account by id.
    ///
    /// `created_at` is preserved on update; `last_login_at` is taken from the
    /// input so a login flow can record its own timestamp.
    pub async fn upsert(&self, account: &NewAccount) -> Result<AccountRecord> {
        let now = now_millis();
        sqlx::query(
            "INSERT INTO accounts (
                 id, username, display_name, uid, school_id, user_type, client_id,
                 status, last_login_at, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                 username      = excluded.username,
                 display_name  = excluded.display_name,
                 uid           = excluded.uid,
                 school_id     = excluded.school_id,
                 user_type     = excluded.user_type,
                 client_id     = excluded.client_id,
                 status        = excluded.status,
                 last_login_at = excluded.last_login_at,
                 updated_at    = excluded.updated_at",
        )
        .bind(account.id.as_str())
        .bind(&account.username)
        .bind(&account.display_name)
        .bind(&account.uid)
        .bind(&account.school_id)
        .bind(account.user_type)
        .bind(&account.client_id)
        .bind(&account.status)
        .bind(account.last_login_at)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        self.get(&account.id).await?.ok_or(StoreError::NotFound)
    }

    /// Read one account.
    pub async fn get(&self, id: &AccountId) -> Result<Option<AccountRecord>> {
        let row = sqlx::query(
            "SELECT id, username, display_name, uid, school_id, user_type, client_id,
                    status, last_login_at, created_at, updated_at
             FROM accounts WHERE id = ?",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_account).transpose()
    }

    /// All accounts, most recently used first.
    pub async fn list(&self) -> Result<Vec<AccountRecord>> {
        let rows = sqlx::query(
            "SELECT id, username, display_name, uid, school_id, user_type, client_id,
                    status, last_login_at, created_at, updated_at
             FROM accounts
             ORDER BY last_login_at DESC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(map_account).collect()
    }

    /// Delete one account. Returns whether a row existed.
    pub async fn delete(&self, id: &AccountId) -> Result<bool> {
        let result = sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Record a successful login without touching the rest of the record.
    pub async fn mark_login(&self, id: &AccountId, at: i64) -> Result<bool> {
        let result =
            sqlx::query("UPDATE accounts SET last_login_at = ?, updated_at = ? WHERE id = ?")
                .bind(at)
                .bind(now_millis())
                .bind(id.as_str())
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

fn map_account(row: &sqlx::sqlite::SqliteRow) -> Result<AccountRecord> {
    Ok(AccountRecord {
        id: AccountId::from(row.try_get::<String, _>("id")?),
        username: row.try_get("username")?,
        display_name: row.try_get("display_name")?,
        uid: row.try_get("uid")?,
        school_id: row.try_get("school_id")?,
        user_type: row.try_get("user_type")?,
        client_id: row.try_get("client_id")?,
        status: row.try_get("status")?,
        last_login_at: row.try_get("last_login_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    async fn db() -> Database {
        Database::memory().await.unwrap()
    }

    #[tokio::test]
    async fn upsert_inserts_then_updates() {
        let db = db().await;
        let repo = db.accounts();

        let mut account = NewAccount::new("acc-1");
        account.username = Some("alice".into());
        account.display_name = Some("Alice".into());
        let created = repo.upsert(&account).await.unwrap();
        assert_eq!(created.username.as_deref(), Some("alice"));
        assert_eq!(created.status, "active");

        account.display_name = Some("Alice Liddell".into());
        account.user_type = Some(1);
        account.last_login_at = Some(1_700_000_000_000);
        let updated = repo.upsert(&account).await.unwrap();

        assert_eq!(updated.display_name.as_deref(), Some("Alice Liddell"));
        assert_eq!(updated.user_type, Some(1));
        assert_eq!(updated.created_at, created.created_at);
        assert!(updated.updated_at >= created.updated_at);
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_missing_is_none_and_delete_reports_existence() {
        let db = db().await;
        let repo = db.accounts();
        let id = AccountId::from("acc-1");

        assert!(repo.get(&id).await.unwrap().is_none());
        repo.upsert(&NewAccount::new(id.clone())).await.unwrap();
        assert!(repo.delete(&id).await.unwrap());
        assert!(!repo.delete(&id).await.unwrap());
        assert!(repo.get(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn mark_login_only_updates_existing_accounts() {
        let db = db().await;
        let repo = db.accounts();
        let id = AccountId::from("acc-1");
        repo.upsert(&NewAccount::new(id.clone())).await.unwrap();

        assert!(repo.mark_login(&id, 42).await.unwrap());
        assert_eq!(
            repo.get(&id).await.unwrap().unwrap().last_login_at,
            Some(42)
        );
        assert!(!repo
            .mark_login(&AccountId::from("missing"), 42)
            .await
            .unwrap());
    }
}
