//! Tauri application entry point.
//!
//! This crate is intentionally thin: it wires up state and registers commands.
//! Business logic lives in the `up366-*` crates.

mod commands;
mod confirm;
mod error;
mod session;
mod state;

pub use confirm::{Confirmations, PendingWrite, CONFIRM_TTL};
pub use error::AppError;
pub use session::AppState;
pub use state::{BookView, CourseView, JobView, StatusView};

/// Build the application: managed state plus every command the UI calls.
///
/// Takes an already-open [`AppState`] so tests can construct the app without a
/// Tauri runtime, and so failures to open the database happen at a place that
/// can report them.
pub fn app(state: AppState) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::accounts::accounts_list,
            commands::accounts::account_login,
            commands::accounts::account_switch,
            commands::accounts::account_logout,
            commands::books::books_list,
            commands::books::book_detail,
            commands::books::book_task_get,
            commands::catalog::courses_list,
            commands::catalog::jobs_list,
            commands::correction::correction_overview,
            commands::correction::correction_draw,
            commands::correction::correction_submit,
            commands::correction::correction_status,
            commands::homework::answer_get,
            commands::writing::applied_writing_get,
            commands::writing::applied_writing_preview,
            commands::writing::applied_writing_submit,
            commands::system::settings_get,
            commands::system::settings_update,
            commands::system::cache_clear,
            commands::wordpk::wordpk_preview,
            commands::wordpk::wordpk_submit,
            commands::readaloud::readaloud_get,
            commands::readaloud::readaloud_upload,
            commands::readaloud::readaloud_preview,
            commands::readaloud::readaloud_submit,
            commands::readaloud::book_readaloud_get,
            commands::readaloud::book_readaloud_upload,
            commands::readaloud::book_readaloud_preview,
        ])
}

/// Run the desktop application.
pub fn run() {
    let state = tauri::async_runtime::block_on(open_state());

    app(state)
        .run(tauri::generate_context!())
        .expect("failed to start the up366 application");
}

/// Open persistent state, falling back to an in-memory database when the
/// on-disk one cannot be opened.
async fn open_state() -> AppState {
    match AppState::open().await {
        Ok(state) => state,
        Err(_) => fallback_state().await,
    }
}

/// A failed open must not crash the app to a blank screen; in-memory state
/// still lets the user log in and see errors.
async fn fallback_state() -> AppState {
    let db = up366_store::Database::memory().await.expect("in-memory sqlite");
    AppState::with_database(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback runs inside the runtime started by `run()`; it must not
    /// try to start a second runtime (Tokio panics on a nested `block_on`).
    #[tokio::test]
    async fn fallback_state_opens_inside_an_existing_runtime() {
        let state = fallback_state().await;
        assert!(state.active_id().is_none());
    }
}
