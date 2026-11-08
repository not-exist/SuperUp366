//! Account commands: login, switch, list, forget.

use up366_core::AccountId;
use up366_protocol::{login, Client, Session};
use up366_store::NewAccount;

use crate::error::{AppError, CommandResult};
use crate::session::AppState;
use crate::state::AccountView;

/// Account id derived from the login name.
///
/// The username is the only stable identifier the login response guarantees, so
/// it doubles as the local account id. Normalising it keeps the same account
/// resolving to the same row regardless of case.
pub fn account_id_for(username: &str) -> AccountId {
    AccountId::from(username.trim().to_lowercase())
}

/// Build the view the account switcher renders.
fn account_view(
    record: &up366_store::AccountRecord,
    active: Option<&AccountId>,
    session_valid: bool,
) -> AccountView {
    let username = record
        .username
        .clone()
        .unwrap_or_else(|| record.id.to_string());
    AccountView {
        account_id: record.id.to_string(),
        display_name: record
            .display_name
            .clone()
            .unwrap_or_else(|| username.clone()),
        school_name: String::new(),
        username,
        active: active == Some(&record.id),
        session_valid: session_valid && record.status != "needs_reauth",
        expires_at: None,
    }
}

/// List every known account, most recently used first.
#[tauri::command(rename_all = "snake_case")]
pub async fn accounts_list(state: tauri::State<'_, AppState>) -> CommandResult<Vec<AccountView>> {
    let active = state.active_id();
    let mut views = Vec::new();
    for record in state.db.accounts().list().await.map_err(storage_error)? {
        // An expired session stays on disk so the account is not forgotten,
        // but it must not be offered as usable.
        let session_valid = state
            .secrets
            .load(&record.id)
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str::<Session>(&json).ok())
            .map(|session| !session.is_expired())
            .unwrap_or(false);
        views.push(account_view(&record, active.as_ref(), session_valid));
    }
    Ok(views)
}

/// Log in with a username and password.
///
/// The password is used for exactly one upstream call and is then dropped: the
/// returned session holds a TGT and cookies, never the password.
#[tauri::command(rename_all = "snake_case")]
pub async fn account_login(
    username: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AccountView> {
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::new("invalid_input", "账号不能为空"));
    }
    if password.is_empty() {
        return Err(AppError::new("invalid_input", "密码不能为空"));
    }

    // Reuse the device id of a previous session when the account is known, so a
    // re-login does not look like a new device to the server.
    let account_id = account_id_for(username);
    let existing_client_id = state
        .db
        .accounts()
        .get(&account_id)
        .await
        .map_err(storage_error)?
        .and_then(|record| record.client_id);

    let client = match existing_client_id {
        Some(client_id) => Client::with_client_id(client_id).map_err(AppError::from)?,
        None => Client::new().map_err(AppError::from)?,
    };

    let session: Session = login(&client, username, &password)
        .await
        .map_err(AppError::from)?;
    // The password outlives this function's scope only as this plain binding;
    // dropping it here is the last thing the function does with it.
    drop(password);

    let now = up366_store::now_millis();
    let record = state
        .db
        .accounts()
        .upsert(&NewAccount {
            id: account_id.clone(),
            username: Some(username.to_owned()),
            display_name: None,
            uid: None,
            school_id: None,
            user_type: session.user_type,
            client_id: Some(session.client_id.clone()),
            status: "active".to_owned(),
            last_login_at: Some(now),
        })
        .await
        .map_err(storage_error)?;

    state
        .save_session(&account_id, &session)
        .map_err(storage_error)?;
    state.install(account_id.clone(), client, session);

    let stored = state
        .db
        .accounts()
        .get(&account_id)
        .await
        .map_err(storage_error)?
        .unwrap_or(record);
    Ok(account_view(&stored, Some(&account_id), true))
}

/// Switch to a previously logged-in account.
///
/// Restores the stored session, so no password is needed. A missing or expired
/// session surfaces as an auth error naming the account.
#[tauri::command(rename_all = "snake_case")]
pub async fn account_switch(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<AccountView> {
    let account_id = AccountId::from(account_id.as_str());
    let record = state
        .db
        .accounts()
        .get(&account_id)
        .await
        .map_err(storage_error)?
        .ok_or_else(|| AppError::new("not_found", "账号不存在"))?;

    let session_json = state
        .secrets
        .load(&account_id)
        .map_err(storage_error)?
        .ok_or_else(AppError::auth_required)?;
    let session: Session = serde_json::from_str(&session_json)
        .map_err(|error| AppError::new("storage_error", format!("会话数据损坏: {error}")))?;
    // The stored ticket was rejected upstream on an earlier run: require the
    // password again instead of installing a client that can only fail.
    if session.is_expired() {
        return Err(AppError::auth_required());
    }

    let client = Client::with_session(session.clone()).map_err(AppError::from)?;
    state.install(account_id.clone(), client, session.clone());

    state
        .db
        .accounts()
        .mark_login(&account_id, up366_store::now_millis())
        .await
        .map_err(storage_error)?;

    Ok(account_view(&record, Some(&account_id), true))
}

/// Forget an account: drop its runtime, session and cached data.
#[tauri::command(rename_all = "snake_case")]
pub async fn account_logout(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<()> {
    state.forget(&AccountId::from(account_id.as_str())).await
}

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_ids_are_stable_across_case_and_whitespace() {
        assert_eq!(account_id_for("  Alice "), AccountId::from("alice"));
        assert_eq!(account_id_for("ALICE"), AccountId::from("alice"));
        assert_eq!(account_id_for("alice"), AccountId::from("alice"));
    }
}
