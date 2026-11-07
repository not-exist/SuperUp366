//! Session credential storage.
//!
//! A session is a serialised JSON blob (cookies, `tgt`, device id, ...). It is
//! sensitive and therefore never written to SQLite: [`KeyringStore`] keeps it
//! in the OS credential store, while [`MemoryStore`] exists for tests and
//! headless runs where no credential store is available.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use up366_core::AccountId;

use crate::{Result, StoreError};

/// Service name used for every credential entry.
pub const KEYRING_SERVICE: &str = "cn.up366.superup366";

/// Credential entry name for one account.
pub fn entry_name(account_id: &AccountId) -> String {
    format!("account:{account_id}:session")
}

/// Persists and retrieves serialised session blobs.
pub trait SecretStore: Send + Sync {
    /// Save (or replace) the session blob for an account.
    fn save(&self, account_id: &AccountId, session_json: &str) -> Result<()>;
    /// Load the session blob, or `None` when the account has no stored
    /// session.
    fn load(&self, account_id: &AccountId) -> Result<Option<String>>;
    /// Delete the stored session. Deleting a missing entry is not an error.
    fn delete(&self, account_id: &AccountId) -> Result<()>;
}

/// OS credential-store implementation.
///
/// On Windows this is the Credential Manager, on macOS the Keychain and on
/// Linux the kernel keyring. The platform backend is selected in `Cargo.toml`.
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the service name, mainly so test builds can stay isolated.
    pub fn with_service(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, account_id: &AccountId) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, &entry_name(account_id))?)
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self {
            service: KEYRING_SERVICE.to_owned(),
        }
    }
}

impl SecretStore for KeyringStore {
    fn save(&self, account_id: &AccountId, session_json: &str) -> Result<()> {
        self.entry(account_id)?.set_password(session_json)?;
        Ok(())
    }

    fn load(&self, account_id: &AccountId) -> Result<Option<String>> {
        match self.entry(account_id)?.get_password() {
            Ok(session) => Ok(Some(session)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn delete(&self, account_id: &AccountId) -> Result<()> {
        match self.entry(account_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// In-memory implementation. Clones share the same map.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    entries: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, String>>> {
        self.entries
            .lock()
            .map_err(|_| StoreError::Other("credential store lock poisoned".to_owned()))
    }
}

impl SecretStore for MemoryStore {
    fn save(&self, account_id: &AccountId, session_json: &str) -> Result<()> {
        self.lock()?
            .insert(account_id.as_str().to_owned(), session_json.to_owned());
        Ok(())
    }

    fn load(&self, account_id: &AccountId) -> Result<Option<String>> {
        Ok(self.lock()?.get(account_id.as_str()).cloned())
    }

    fn delete(&self, account_id: &AccountId) -> Result<()> {
        self.lock()?.remove(account_id.as_str());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes() {
        let store = MemoryStore::new();
        let account = AccountId::from("acc-1");
        let other = AccountId::from("acc-2");

        assert!(store.load(&account).unwrap().is_none());
        store.save(&account, r#"{"tgt":"abc"}"#).unwrap();
        store.save(&other, r#"{"tgt":"def"}"#).unwrap();

        assert_eq!(
            store.load(&account).unwrap().as_deref(),
            Some(r#"{"tgt":"abc"}"#)
        );
        store.save(&account, r#"{"tgt":"new"}"#).unwrap();
        assert_eq!(
            store.load(&account).unwrap().as_deref(),
            Some(r#"{"tgt":"new"}"#)
        );

        store.delete(&account).unwrap();
        assert!(store.load(&account).unwrap().is_none());
        assert!(store.load(&other).unwrap().is_some());
        // Deleting an absent entry is fine.
        store.delete(&account).unwrap();
    }

    #[test]
    fn clones_share_the_same_entries() {
        let store = MemoryStore::new();
        let clone = store.clone();
        let account = AccountId::from("acc-1");

        store.save(&account, "session").unwrap();
        assert_eq!(clone.load(&account).unwrap().as_deref(), Some("session"));
    }

    #[test]
    fn entry_name_is_stable() {
        assert_eq!(entry_name(&AccountId::from(42)), "account:42:session");
        assert_eq!(KEYRING_SERVICE, "cn.up366.superup366");
    }
}
