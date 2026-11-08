//! Settings and cache maintenance commands.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, CommandResult};
use crate::session::AppState;

/// User preferences. Stored as JSON in the `settings` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsView {
    /// Maximum parallel upstream requests for the merged job list.
    pub concurrency: u32,
    /// Per-request timeout in milliseconds.
    pub request_timeout_ms: u64,
    /// Background refresh interval in seconds; `0` disables it.
    pub refresh_interval_sec: u64,
    /// Keeps the read-aloud submit button disabled until the protocol probe
    /// confirms the submission payload.
    pub read_aloud_dry_run: bool,
    /// Whether uploads of local recordings are kept after submission.
    pub keep_audio_files: bool,
}

impl Default for SettingsView {
    fn default() -> Self {
        Self {
            concurrency: 6,
            request_timeout_ms: 20_000,
            refresh_interval_sec: 120,
            read_aloud_dry_run: true,
            keep_audio_files: false,
        }
    }
}

const SETTINGS_KEY: &str = "app.settings";

/// Read settings, falling back to defaults on a missing or unreadable value.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_get(state: tauri::State<'_, AppState>) -> CommandResult<SettingsView> {
    let raw = state
        .db
        .settings()
        .get(SETTINGS_KEY)
        .await
        .map_err(storage_error)?;
    match raw {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(SettingsView::default()),
    }
}

/// Persist settings.
#[tauri::command(rename_all = "snake_case")]
pub async fn settings_update(
    settings: SettingsView,
    state: tauri::State<'_, AppState>,
) -> CommandResult<SettingsView> {
    if settings.concurrency == 0 {
        return Err(AppError::new("invalid_input", "并发数必须大于 0"));
    }
    let json = serde_json::to_string(&settings)
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    state
        .db
        .settings()
        .set(SETTINGS_KEY, &json)
        .await
        .map_err(storage_error)?;
    Ok(settings)
}

/// Drop cached courses and jobs so the next list request refetches.
///
/// Account metadata and stored sessions are deliberately untouched.
#[tauri::command(rename_all = "snake_case")]
pub async fn cache_clear(state: tauri::State<'_, AppState>) -> CommandResult<()> {
    let account_id = state.active_id().ok_or_else(AppError::auth_required)?;
    state
        .db
        .cache()
        .delete_jobs(&account_id)
        .await
        .map_err(storage_error)?;
    state
        .db
        .cache()
        .delete_courses(&account_id)
        .await
        .map_err(storage_error)?;
    Ok(())
}

fn storage_error(error: up366_store::StoreError) -> AppError {
    AppError::new("storage_error", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_the_unverified_read_aloud_submit_off() {
        let settings = SettingsView::default();
        assert!(settings.read_aloud_dry_run);
        assert_eq!(settings.concurrency, 6);
    }

    #[test]
    fn settings_round_trip_through_json() {
        let json = serde_json::to_string(&SettingsView::default()).unwrap();
        let back: SettingsView = serde_json::from_str(&json).unwrap();
        assert_eq!(back, SettingsView::default());
    }

    #[test]
    fn a_corrupt_row_falls_back_to_defaults() {
        let restored: SettingsView = serde_json::from_str("{ not json").unwrap_or_default();
        assert_eq!(restored, SettingsView::default());
    }
}
