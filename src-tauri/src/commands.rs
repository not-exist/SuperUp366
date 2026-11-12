//! Tauri command surface.
//!
//! One module per feature. `lib.rs` refers to commands by their full path so
//! each feature keeps its own namespace. Commands stay thin: read the active
//! account from state, call the protocol adapters, map to a view type.
//!
//! A command never builds an upstream request by hand and never claims a
//! success the server did not confirm.

use up366_core::AccountId;
use up366_protocol::Client;

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

pub mod accounts;
pub mod books;
pub mod catalog;
pub mod correction;
pub mod homework;
pub mod readaloud;
pub mod system;
pub mod wordpk;

/// The numeric student id the endpoints expect.
///
/// The local account id is the lower-cased username, not a uid. The stored
/// profile usually has it; otherwise the user API is asked once per call.
pub(crate) async fn uid_for(
    state: &AppState,
    account_id: &AccountId,
    client: &Client,
) -> CommandResult<String> {
    if let Some(account) = state
        .db
        .accounts()
        .get(account_id)
        .await
        .map_err(|error| AppError::new("storage_error", error.to_string()))?
    {
        if let Some(uid) = account.uid.filter(|uid| !uid.trim().is_empty()) {
            return Ok(uid);
        }
    }
    up366_protocol::adapters::current_user(client)
        .await
        .map(|user| user.uid)
        .map_err(|error| state.session_error(account_id, client, error))
}
