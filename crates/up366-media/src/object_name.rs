//! OSS object names.
//!
//! Confirmed rule (see `docs/protocol/confirmed.md`):
//!
//! ```text
//! storage/{user_type}/study/{class}/{yyyy/mm/dd}/{uid}/{file}
//! ```
//!
//! `user_type` is the upstream `uType` (for example `102` for a student),
//! `uid` is the upstream user id and the date is the **local** upload date,
//! matching the client's `buildOSSObjectName`. `class` is the client's
//! extension class: audio files live under `a`.

use chrono::{DateTime, TimeZone};

/// Extension class used by the client's `$At` table.
fn ext_class(file: &str) -> &'static str {
    let ext = file
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "mp3" | "wav" | "aac" | "flac" | "ogg" | "wma" | "m4a" | "opus" | "amr" => "a",
        _ => "unknown",
    }
}

/// Build the object name for an uploaded recording.
pub fn object_name<Tz: TimeZone>(
    user_type: impl std::fmt::Display,
    uid: &str,
    file: &str,
    at: DateTime<Tz>,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format!(
        "storage/{user_type}/study/{}/{}/{uid}/{file}",
        ext_class(file),
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
            object_name(102, "39370562", "answer.mp3", at(2026, 11, 3)),
            "storage/102/study/a/2026/11/03/39370562/answer.mp3"
        );
    }

    #[test]
    fn zero_pads_month_and_day() {
        assert_eq!(
            object_name(2, "u1", "a.mp3", at(2025, 1, 2)),
            "storage/2/study/a/2025/01/02/u1/a.mp3"
        );
    }

    #[test]
    fn class_comes_from_the_extension() {
        assert_eq!(
            object_name("102", "u1", "a.WAV", at(2024, 12, 31)),
            "storage/102/study/a/2024/12/31/u1/a.WAV"
        );
        assert_eq!(
            object_name("102", "u1", "notes.txt", at(2024, 12, 31)),
            "storage/102/study/unknown/2024/12/31/u1/notes.txt"
        );
    }

    #[test]
    fn local_offset_is_used_for_the_date() {
        let at = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 9, 0, 30, 0)
            .unwrap();
        assert_eq!(
            object_name(102, "u1", "a.mp3", at),
            "storage/102/study/a/2026/11/09/u1/a.mp3"
        );
    }
}
