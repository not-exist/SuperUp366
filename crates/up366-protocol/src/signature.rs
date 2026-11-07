//! Request signatures.
//!
//! Only three calls carry a signature (`ut`): login, SMS code and task-score
//! submission. The pattern is always
//!
//! ```text
//! ts     = md5(milliseconds)
//! ut     = ts[..10] + md5(payload_material + keyword + ts) + ts[10..]
//! ```
//!
//! `recon/api.md` and `tools/*.py` disagree on two details (the password salt
//! and whether the task-score digest is nested). Both candidates are exposed;
//! the probe step decides which one the live server accepts, and a fixture
//! test then pins the winner. Nothing here guesses.

use md5::{Digest, Md5};

/// Salt used by `recon/api.md` and the old Python client.
pub const PASSWORD_SALT_AT: &str = "fa&s*l%$k!fq$k!ld@fjlk";
/// Salt used by several scripts under `tools/` on the `main` branch.
pub const PASSWORD_SALT_COLON: &str = "fa&s*l%$k!fq$k!ld:fjlk";

const LOGIN_UT_KEYWORD: &str = "loginEncryptionParameter";
const SUBMIT_TASK_KEYWORD: &str = "submitTaskToken-pc-987654";

/// Lowercase hex MD5 of the given bytes.
pub fn md5_hex(bytes: &[u8]) -> String {
    let digest = Md5::digest(bytes);
    let mut out = String::with_capacity(32);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn md5_text(text: &str) -> String {
    md5_hex(text.as_bytes())
}

/// `md5(md5(password) + salt)` — the value sent as `password`.
pub fn login_password_hash(password: &str, salt: &str) -> String {
    md5_text(&format!("{}{salt}", md5_text(password)))
}

/// Login signature for a given millisecond timestamp.
pub fn login_ut(username: &str, password_hash: &str, timestamp_ms: u128) -> String {
    let ts = md5_text(&timestamp_ms.to_string());
    let middle = md5_text(&format!(
        "{username}{password_hash}{LOGIN_UT_KEYWORD}{ts}"
    ));
    format!("{}{middle}{}", &ts[..10], &ts[10..])
}

/// Task-score signature, nested form: `md5(md5(tasksJson) + keyword + ts)`.
pub fn submit_task_ut(tasks_json: &str, timestamp_ms: u128) -> String {
    let ts = md5_text(&timestamp_ms.to_string());
    let middle = md5_text(&format!(
        "{}{SUBMIT_TASK_KEYWORD}{ts}",
        md5_text(tasks_json)
    ));
    format!("{}{middle}{}", &ts[..10], &ts[10..])
}

/// Task-score signature, flat form: `md5(tasksJson + keyword + ts)`.
///
/// Present because the archive contradicts itself; see module docs.
pub fn submit_task_ut_flat(tasks_json: &str, timestamp_ms: u128) -> String {
    let ts = md5_text(&timestamp_ms.to_string());
    let middle = md5_text(&format!("{tasks_json}{SUBMIT_TASK_KEYWORD}{ts}"));
    format!("{}{middle}{}", &ts[..10], &ts[10..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_known_vector() {
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn password_hash_is_double_md5_with_salt() {
        let salt = PASSWORD_SALT_AT;
        let expected = md5_hex(format!("{}{}", md5_hex(b"secret"), salt).as_bytes());
        assert_eq!(login_password_hash("secret", salt), expected);
        assert_ne!(
            login_password_hash("secret", PASSWORD_SALT_AT),
            login_password_hash("secret", PASSWORD_SALT_COLON)
        );
    }

    #[test]
    fn ut_is_64_hex_chars_and_starts_with_ts_prefix() {
        let ut = login_ut("alice", "hash", 1_700_000_000_000);
        assert_eq!(ut.len(), 64);
        assert!(ut.chars().all(|c| c.is_ascii_hexdigit()));

        let ts = md5_text("1700000000000");
        assert!(ut.starts_with(&ts[..10]));
        assert!(ut.ends_with(&ts[10..]));
    }

    #[test]
    fn ut_is_deterministic() {
        assert_eq!(
            login_ut("alice", "hash", 42),
            login_ut("alice", "hash", 42)
        );
    }

    #[test]
    fn task_ut_shares_prefix_and_suffix_with_login_ut() {
        let a = submit_task_ut("[{\"a\":1}]", 99);
        let b = submit_task_ut_flat("[{\"a\":1}]", 99);
        assert_eq!(a.len(), 64);
        assert_eq!(b.len(), 64);
        // Same ts, so prefix and suffix are identical; only the middle differs.
        assert_eq!(a[..10], b[..10]);
        assert_eq!(a[54..], b[54..]);
        assert_ne!(a, b);
    }
}