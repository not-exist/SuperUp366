//! One-shot confirmation tokens for write commands.
//!
//! A write is never issued directly by the UI. The flow is:
//!
//! 1. a `*_preview` command builds the exact request and returns it together
//!    with a token;
//! 2. the UI shows that payload and asks the user to confirm;
//! 3. the `*_submit` command takes the token, and the token is consumed on use.
//!
//! That shape makes a double click, a stale tab or a replayed event unable to
//! submit twice: the second call fails with `confirmation_expired` rather than
//! replaying a write. This matters because upstream rejects duplicates with
//! `-13` and a burned attempt cannot be un-burned.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use up366_core::AccountId;
use uuid::Uuid;

/// How long a preview stays confirmable. Short enough that a forgotten tab
/// cannot submit much later, long enough for a human to read the preview.
pub const CONFIRM_TTL: Duration = Duration::from_secs(120);

/// A pending write: which account it belongs to and the operation it describes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PendingWrite {
    pub account_id: AccountId,
    /// Opaque label the UI can show while confirming, e.g. `word_pk`.
    pub operation: &'static str,
    /// The decoded request, rendered in the confirmation dialog.
    pub payload: serde_json::Value,
}

/// Store of pending writes. Clones share the same entries.
#[derive(Debug, Clone, Default)]
pub struct Confirmations {
    entries: std::sync::Arc<Mutex<HashMap<String, (PendingWrite, Instant)>>>,
}

impl Confirmations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending write and return its token.
    ///
    /// The token is random rather than derived from the payload: two identical
    /// previews (for example the same score submitted twice after a typo) must
    /// not share a token, and a guessable token would be a way to skip the
    /// confirmation dialog.
    pub fn issue(&self, write: PendingWrite) -> String {
        let token = Uuid::new_v4().simple().to_string();
        self.lock().insert(token.clone(), (write, Instant::now()));
        token
    }

    /// Consume a token, failing if it is unknown, expired or already used.
    pub fn consume(&self, account_id: &AccountId, token: &str) -> Result<PendingWrite, Write> {
        let mut entries = self.lock();
        let Some((write, issued_at)) = entries.get(token).cloned() else {
            return Err(Write::ConfirmationExpired);
        };
        if issued_at.elapsed() > CONFIRM_TTL {
            entries.remove(token);
            return Err(Write::ConfirmationExpired);
        }
        if write.account_id != *account_id {
            return Err(Write::AccountMismatch);
        }
        entries.remove(token);
        Ok(write)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, (PendingWrite, Instant)>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Why a write was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// The token is unknown, stale or already consumed.
    ConfirmationExpired,
    /// The token belongs to a different account.
    AccountMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(account: &str) -> PendingWrite {
        PendingWrite {
            account_id: AccountId::from(account),
            operation: "word_pk",
            payload: serde_json::json!({ "score": 8 }),
        }
    }

    #[test]
    fn a_token_can_only_be_used_once() {
        let store = Confirmations::new();
        let account = AccountId::from("a");
        let token = store.issue(write("a"));

        assert!(store.consume(&account, &token).is_ok());
        assert_eq!(
            store.consume(&account, &token),
            Err(Write::ConfirmationExpired)
        );
    }

    #[test]
    fn a_token_belongs_to_one_account() {
        let store = Confirmations::new();
        let token = store.issue(write("a"));

        assert_eq!(
            store.consume(&AccountId::from("b"), &token),
            Err(Write::AccountMismatch)
        );
        // The rejected attempt must not burn the token for the right account.
        assert!(store.consume(&AccountId::from("a"), &token).is_ok());
    }

    #[test]
    fn an_unknown_token_is_expired_not_a_crash() {
        let store = Confirmations::new();
        assert_eq!(
            store.consume(&AccountId::from("a"), "not-a-token"),
            Err(Write::ConfirmationExpired)
        );
    }

    #[test]
    fn issued_tokens_are_distinct() {
        let store = Confirmations::new();
        let first = store.issue(write("a"));
        let second = store.issue(write("a"));
        assert_ne!(first, second);
    }
}
