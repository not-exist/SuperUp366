//! Deadline parsing.
//!
//! `endTime` arrives in three shapes across the upstream hosts: an ISO-8601
//! string with or without a `Z` suffix, and an epoch-millisecond number sent as
//! a string. Anything unparseable is treated as "no deadline" rather than as an
//! error, because a malformed date must never make the list unusable.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

/// Parse an `endTime` value into an aware UTC timestamp.
///
/// Returns `None` for empty, missing or unparseable input.
pub fn parse_deadline(value: &str) -> Option<DateTime<Utc>> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }

    // Epoch milliseconds arrive as a plain digit string.
    if let Ok(millis) = text.parse::<i64>() {
        if let Some(dt) = DateTime::from_timestamp_millis(millis) {
            return Some(dt);
        }
        return None;
    }

    // `Z` is accepted by fromisoformat in recent Python but not by chrono.
    let normalized = text.strip_suffix('Z').unwrap_or(text);
    parse_naive_or_offset(normalized).or_else(|| {
        // Some rows use a space instead of `T`.
        normalized
            .replacen(' ', "T", 1)
            .as_str()
            .pipe(parse_naive_or_offset)
    })
}

fn parse_naive_or_offset(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(text, format) {
            return Some(naive.and_utc());
        }
        if let Ok(date) = NaiveDate::parse_from_str(text, format) {
            return Some(date.and_hms_opt(0, 0, 0)?.and_utc());
        }
    }
    None
}

/// Small local helper so the space/`T` fallback reads declaratively.
trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R
    where
        R: Sized,
    {
        f(self)
    }
}
impl Pipe for &str {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parses_iso_with_and_without_z() {
        let expected = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        assert_eq!(parse_deadline("2026-02-01T00:00:00Z"), Some(expected));
        assert_eq!(parse_deadline("2026-02-01T00:00:00+00:00"), Some(expected));
    }

    #[test]
    fn parses_epoch_millis() {
        let expected = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let millis = expected.timestamp_millis();
        assert_eq!(parse_deadline(&millis.to_string()), Some(expected));
    }

    #[test]
    fn parses_space_separated_and_date_only() {
        let space = parse_deadline("2026-02-01 08:30:00");
        let date = parse_deadline("2026-02-01");
        assert_eq!(
            space.unwrap().naive_utc(),
            NaiveDate::from_ymd_opt(2026, 2, 1)
                .unwrap()
                .and_hms_opt(8, 30, 0)
                .unwrap()
        );
        assert_eq!(
            date.unwrap().naive_utc(),
            NaiveDate::from_ymd_opt(2026, 2, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        );
    }

    #[test]
    fn unparseable_input_is_none_not_an_error() {
        for value in ["", "  ", "not a date", "abc123", "2026-13-45", "12/31/26"] {
            assert_eq!(parse_deadline(value), None, "{value:?}");
        }
    }

    #[test]
    fn epoch_sentinels_parse_instead_of_failing() {
        // Any digit string is a valid epoch-ms value, including sentinels that
        // mean "long expired". Failing them would break the whole list.
        assert_eq!(parse_deadline("0"), DateTime::from_timestamp_millis(0));
        assert_eq!(parse_deadline("-1"), DateTime::from_timestamp_millis(-1));
    }

    #[test]
    fn future_and_past_comparisons_are_absolute() {
        let now = Utc.with_ymd_and_hms(2026, 1, 10, 12, 0, 0).unwrap();
        let past = parse_deadline("2026-01-01T00:00:00Z").unwrap();
        let future = parse_deadline("2026-02-01T00:00:00Z").unwrap();
        assert!(past < now);
        assert!(future > now);
    }
}
