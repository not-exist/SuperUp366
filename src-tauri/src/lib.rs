//! Tauri application entry point.
//!
//! This crate is intentionally thin: it wires plugins, manages state and
//! registers commands. Business logic lives in the `up366-*` crates.

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to start the up366 application");
}
