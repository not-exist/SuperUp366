//! Protocol layer: endpoint table, wire encoding and request signatures.
//!
//! The HTTP client itself lives in [`client`] (added in the transport step);
//! everything in this module is pure and unit-testable without a network.
//!
//! Ground truth for the tables here is `recon/api.md` on the `main` branch.
//! Where the archive contradicts itself the conflict is recorded in
//! `docs/protocol/to-probe.md` and the code exposes every candidate instead of
//! silently picking one.

pub mod endpoint;
pub mod form;
pub mod signature;

pub use endpoint::{get, BodyKind, Endpoint, ENDPOINTS};
pub use form::{encode_form, normalize_key};
pub use signature::{
    login_password_hash, login_ut, submit_task_ut, submit_task_ut_flat, PASSWORD_SALT_AT,
    PASSWORD_SALT_COLON,
};
