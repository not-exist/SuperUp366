//! Tauri command surface.
//!
//! One module per feature. `lib.rs` refers to commands by their full path so
//! each feature keeps its own namespace. Commands stay thin: read the active
//! account from state, call the protocol adapters, map to a view type.
//!
//! A command never builds an upstream request by hand and never claims a
//! success the server did not confirm.

pub mod accounts;
pub mod catalog;
pub mod homework;
pub mod system;
