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
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::accounts::accounts_list,
            commands::accounts::account_login,
            commands::accounts::account_switch,
            commands::accounts::account_logout,
            commands::books::books_list,
            commands::books::book_detail,
            commands::catalog::courses_list,
            commands::catalog::jobs_list,
            commands::correction::correction_overview,
            commands::correction::correction_draw,
            commands::correction::correction_submit,
            commands::correction::correction_status,
            commands::homework::answer_get,
            commands::system::settings_get,
            commands::system::settings_update,
            commands::system::cache_clear,
            commands::wordpk::wordpk_preview,
            commands::wordpk::wordpk_submit,
            commands::readaloud::readaloud_get,
            commands::readaloud::readaloud_upload,
            commands::readaloud::readaloud_submit,
        ])
}

/// Run the desktop application.
pub fn run() {
    let state = tauri::async_runtime::block_on(async {
        AppState::open().await.unwrap_or_else(|_| {
            AppState::with_database(
                // A failed open must not crash the app to a blank screen; fall
                // back to in-memory state so the user can log in and see errors.
                tauri::async_runtime::block_on(up366_store::Database::memory())
                    .expect("in-memory sqlite"),
            )
        })
    });

    app(state)
        .run(tauri::generate_context!())
        .expect("failed to start the up366 application");
}
