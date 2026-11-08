//! Application state: one account context and its database.
//!
//! Two invariants live here and are enforced by construction:
//!
//! 1. **No plaintext password.** Only an upstream [`Session`] (TGT, cookies,
//!    device id) is ever held, and it is persisted through the OS credential
//!    store rather than SQLite.
//! 2. **One account at a time.** Each account gets its own [`Client`], so
//!    switching accounts never borrows another account's cookies.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use up366_core::AccountId;
use up366_protocol::{Client, Session};
use up366_store::{MemoryStore, SecretStore};

use crate::confirm::Confirmations;

/// A live session for one account.
pub struct AccountRuntime {
    /// HTTP client carrying this account's cookies and TGT.
    pub client: Client,
    /// The session this client was built from, re-persisted after cookie churn.
    pub session: Session,
}

/// Everything the Tauri commands need, shared as Tauri-managed state.
pub struct AppState {
    /// Account metadata and cached upstream data.
    pub db: up366_store::Database,
    /// Credential store; in-memory in tests and headless runs.
    pub secrets: Arc<dyn SecretStore>,
    /// The account currently in use, or `None` before login.
    pub active: RwLock<Option<AccountId>>,
    /// Per-account HTTP clients.
    pub runtimes: RwLock<HashMap<AccountId, AccountRuntime>>,
    /// Pending writes awaiting explicit confirmation.
    pub confirmations: Confirmations,
}

impl AppState {
    /// Open the production database and use the OS credential store.
    pub async fn open() -> up366_store::Result<Self> {
        let db = up366_store::Database::open(default_db_path()).await?;
        // Production uses the OS keyring; `with_database` exists for tests and
        // headless runs where no credential store is available.
        Ok(Self::with_secrets(
            db,
            Arc::new(up366_store::KeyringStore::new()),
        ))
    }

    /// Build state around an existing database, using the OS credential store.
    pub fn with_database(db: up366_store::Database) -> Self {
        Self::with_secrets(db, Arc::new(MemoryStore::new()))
    }

    /// Build state with an explicit credential store (tests use memory).
    pub fn with_secrets(db: up366_store::Database, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            db,
            secrets,
            active: RwLock::new(None),
            runtimes: RwLock::new(HashMap::new()),
            confirmations: Confirmations::new(),
        }
    }

    /// The account currently selected, if any.
    pub fn active_id(&self) -> Option<AccountId> {
        self.active.read().clone()
    }

    /// Insert (or replace) the runtime for an account and make it active.
    pub fn install(&self, account_id: AccountId, client: Client, session: Session) {
        self.runtimes
            .write()
            .insert(account_id.clone(), AccountRuntime { client, session });
        self.active.write().replace(account_id);
    }

    /// The active account and its client, or an auth error when none is active.
    ///
    /// `requested` is the account the caller says it is acting for. It must
    /// match the active account: the alternative is a stale frontend query
    /// silently reading another account's data.
    pub fn require_client(
        &self,
        requested: Option<&str>,
    ) -> crate::error::CommandResult<(AccountId, Client)> {
        let account_id = self
            .active_id()
            .ok_or_else(crate::error::AppError::auth_required)?;
        if let Some(requested) = requested {
            if *account_id.as_str() != *requested {
                return Err(crate::error::AppError::new(
                    "account_mismatch",
                    "请求的账号不是当前账号",
                ));
            }
        }
        let client = self
            .runtimes
            .read()
            .get(&account_id)
            .map(|runtime| runtime.client.clone())
            .ok_or_else(crate::error::AppError::auth_required)?;
        // A client already known to be rejected upstream must not be used for
        // another request: the next answer would be the same dead-ticket error.
        if client.is_expired() {
            return Err(crate::error::AppError::auth_required());
        }
        Ok((account_id, client))
    }

    /// Persist a session for an account in the credential store.
    pub fn save_session(
        &self,
        account_id: &AccountId,
        session: &Session,
    ) -> up366_store::Result<()> {
        let json = serde_json::to_string(session).unwrap_or_default();
        self.secrets.save(account_id, &json)
    }

    /// Convert an upstream failure into the error the UI sees.
    ///
    /// When the failure killed the session (the TGT was rejected and the ticket
    /// exchange failed), the expiry is persisted first: that is what makes the
    /// account switcher ask for a password on the next launch instead of
    /// replaying the same dead ticket, and it replaces the raw upstream text
    /// ("找不到tgt") with the stable `session_expired` code.
    pub fn session_error(
        &self,
        account_id: &AccountId,
        client: &Client,
        error: up366_protocol::ProtocolError,
    ) -> crate::error::AppError {
        if client.is_expired() {
            let _ = self.save_session(account_id, &client.session());
            return crate::error::AppError::auth_required();
        }
        crate::error::AppError::from(error)
    }

    /// Forget an account: drop its runtime, session and cached data.
    ///
    /// Async because it clears the cache rows; credential errors surface first
    /// so a failed deletion is never silently partial.
    pub async fn forget(&self, account_id: &AccountId) -> crate::error::CommandResult<()> {
        let storage = || crate::error::AppError::new("storage_error", "存储操作失败");
        self.secrets.delete(account_id).map_err(|_| storage())?;
        self.db
            .accounts()
            .delete(account_id)
            .await
            .map_err(|_| storage())?;
        self.db
            .cache()
            .delete_jobs(account_id)
            .await
            .map_err(|_| storage())?;
        self.db
            .cache()
            .delete_courses(account_id)
            .await
            .map_err(|_| storage())?;

        self.runtimes.write().remove(account_id);
        if self.active_id().as_ref() == Some(account_id) {
            self.active.write().take();
        }
        Ok(())
    }
}

/// Where the SQLite file lives.
fn default_db_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("superup366")
        .join("up366.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn state() -> AppState {
        let db = up366_store::Database::memory().await.unwrap();
        AppState::with_secrets(db, Arc::new(MemoryStore::new()))
    }

    #[tokio::test]
    async fn no_active_account_requires_login() {
        let state = state().await;
        // `err()` rather than `unwrap_err()`: the ok side holds a `Client`,
        // which is deliberately not `Debug`.
        let code = state.require_client(None).err().map(|error| error.code);
        assert_eq!(code, Some("session_expired"));
        assert!(state.active_id().is_none());
    }

    #[tokio::test]
    async fn installing_a_session_makes_it_active_and_persistable() {
        let state = state().await;
        let account = AccountId::from("acc-1");
        let mut session = Session::new("alice", "TGT-1", "CID-1");
        session
            .cookies
            .push(("CASTGC".to_owned(), "abc".to_owned()));
        let client = Client::with_client_id("CID-1").unwrap();

        state.install(account.clone(), client, session.clone());

        assert_eq!(state.active_id(), Some(account.clone()));
        let (id, _) = state.require_client(None).unwrap();
        assert_eq!(id, account);

        state.save_session(&account, &session).unwrap();
        assert!(state.secrets.load(&account).unwrap().is_some());
    }

    #[tokio::test]
    async fn forgetting_an_account_clears_session_and_runtime() {
        let state = state().await;
        let account = AccountId::from("acc-2");
        state.install(
            account.clone(),
            Client::with_client_id("CID-2").unwrap(),
            Session::new("bob", "TGT-2", "CID-2"),
        );
        state
            .save_session(&account, &Session::new("bob", "TGT-2", "CID-2"))
            .unwrap();

        state.forget(&account).await.unwrap();

        assert_eq!(state.active_id(), None);
        let code = state.require_client(None).err().map(|error| error.code);
        assert_eq!(code, Some("session_expired"));
        assert!(state.secrets.load(&account).unwrap().is_none());
    }

    #[tokio::test]
    async fn a_rejected_session_is_persisted_and_refused() {
        let state = state().await;
        let account = AccountId::from("acc-expired");
        let client = Client::with_client_id("CID-3").unwrap();
        state.install(
            account.clone(),
            client.clone(),
            Session::new("alice", "TGT-1", "CID-3"),
        );

        // The protocol layer flips this flag when the ticket exchange fails.
        client.mark_expired();
        let error = state.session_error(
            &account,
            &client,
            up366_protocol::ProtocolError::Api {
                code: -23,
                message: "找不到tgt".into(),
            },
        );
        assert_eq!(error.code, "session_expired");

        // The next launch reads the flag from the credential store, and this
        // run refuses to reuse the dead client.
        let stored: Session =
            serde_json::from_str(&state.secrets.load(&account).unwrap().unwrap()).unwrap();
        assert!(stored.is_expired());
        let code = state.require_client(None).err().map(|error| error.code);
        assert_eq!(code, Some("session_expired"));
    }

    #[tokio::test]
    async fn two_accounts_keep_separate_clients() {
        let state = state().await;
        state.install(
            AccountId::from("a"),
            Client::with_client_id("CID-A").unwrap(),
            Session::new("a", "TGT-A", "CID-A"),
        );
        let second = AccountId::from("b");
        state.install(
            second.clone(),
            Client::with_client_id("CID-B").unwrap(),
            Session::new("b", "TGT-B", "CID-B"),
        );

        let (active, client) = state.require_client(None).unwrap();
        assert_eq!(active, second);
        assert_eq!(client.client_id(), "CID-B");
    }
}
