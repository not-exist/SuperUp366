//! Wire encoding for form and query bodies.
//!
//! Confirmed behaviour from `recon/api.md`:
//!
//! * POST bodies are `application/x-www-form-urlencoded`.
//! * Structured values (objects/arrays) are sent as JSON text.
//! * A literal `###` in a field name is rewritten to `.` so callers can express
//!   `pager.pageSize` without ambiguity.
//! * `null` fields are omitted.

use serde_json::Value;

/// Rewrite `pager###pageSize` to `pager.pageSize`.
pub fn normalize_key(key: &str) -> String {
    key.replace("###", ".")
}

/// Encode one value the way the official client does.
///
/// Returns `None` for `null`, which callers drop entirely.
pub fn encode_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

/// Encode a list of fields into ordered `(name, value)` pairs.
///
/// Order is preserved so request fixtures stay byte-comparable.
pub fn encode_form<I, K>(fields: I) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (K, Value)>,
    K: AsRef<str>,
{
    fields
        .into_iter()
        .filter_map(|(key, value)| {
            encode_value(&value).map(|encoded| (normalize_key(key.as_ref()), encoded))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rewrites_hash_separator() {
        assert_eq!(normalize_key("pager###pageSize"), "pager.pageSize");
        assert_eq!(normalize_key("plain"), "plain");
    }

    #[test]
    fn structured_values_become_json_text() {
        let pairs = encode_form([
            ("filter", json!({ "name": "词汇", "page": 1 })),
            ("ids", json!([1, "two"])),
        ]);
        assert_eq!(pairs[0].0, "filter");
        assert_eq!(pairs[0].1, r#"{"name":"词汇","page":1}"#);
        assert_eq!(pairs[1].1, r#"[1,"two"]"#);
    }

    #[test]
    fn null_values_are_dropped_and_ordering_preserved() {
        let pairs = encode_form([
            ("a", json!("1")),
            ("b", Value::Null),
            ("c", json!(3)),
        ]);
        assert_eq!(pairs, vec![("a".into(), "1".into()), ("c".into(), "3".into())]);
    }

    #[test]
    fn booleans_and_numbers_are_plain_text() {
        let pairs = encode_form([("flag", json!(true)), ("n", json!(1.5))]);
        assert_eq!(pairs[0].1, "true");
        assert_eq!(pairs[1].1, "1.5");
    }
}