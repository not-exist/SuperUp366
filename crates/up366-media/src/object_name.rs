//! OSS object names.
//!
//! Confirmed rule (see `docs/protocol/confirmed.md`):
//!
//! ```text
//! storage{user_type}/study/a/{yyyy/mm/dd}/{uid}/{file}
//! ```
//!
//! `user_type` is the upstream `uType` (for example `1` for a student), `uid`
//! is the upstream user id and the date is the upload date in UTC.

use chrono::{DateTime, Utc};

/// Build the object name for an uploaded recording.
pub fn object_name(
    user_type: impl std::fmt::Display,
    uid: &str,
    file: &str,
    at: DateTime<Utc>,
) -> String {
    format!(
        "storage{user_type}/study/a/{}/{uid}/{file}",
        at.format("%Y/%m/%d")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 23, 59, 59).unwrap()
    }

    #[test]
    fn follows_the_confirmed_layout() {
        assert_eq!(
            object_name(1, "12345", "answer.mp3", at(2025, 3, 7)),
            "storage1/study/a/2025/03/07/12345/answer.mp3"
        );
    }

    #[test]
    fn zero_pads_month_and_day() {
        assert_eq!(
            object_name(2, "u1", "a.mp3", at(2025, 1, 2)),
            "storage2/study/a/2025/01/02/u1/a.mp3"
        );
    }

    #[test]
    fn string_user_type_is_supported() {
        assert_eq!(
            object_name("1", "u1", "a.mp3", at(2024, 12, 31)),
            "storage1/study/a/2024/12/31/u1/a.mp3"
        );
    }
}
