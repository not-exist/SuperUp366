//! Response envelope decoding.
//!
//! Every upstream endpoint wraps its payload as
//!
//! ```text
//! { "result": { "code": 0, "msg": "ok" }, "data": <payload> }
//! ```
//!
//! `code == 0` is success for business calls. Login is the one exception and
//! uses `-26`; the service-ticket call uses `-24`. Nothing here guesses: a
//! shape mismatch becomes [`ProtocolError::Decode`], never a panic.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::dto::{Page, Pager, RawPayload};
use crate::error::{ProtocolError, Result};

/// Business success code for ordinary endpoints.
pub const OK_CODE: i64 = 0;

/// Decode a JSON body from raw bytes.
pub fn parse_json(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice(bytes)
        .map_err(|error| ProtocolError::Decode(format!("invalid JSON: {error}")))
}

/// Parse an envelope and return `data` when `result.code == 0`.
///
/// `None` means the envelope had no `data` member (or it was `null`).
pub fn parse_envelope(payload: Value) -> Result<Option<Value>> {
    parse_envelope_with_codes(payload, &[OK_CODE])
}

/// Parse an envelope accepting a specific set of success codes.
///
/// Used for login (`-26`) and service-ticket issuance (`-24`). Any other code
/// is returned as [`ProtocolError::Api`].
pub fn parse_envelope_with_codes(payload: Value, ok_codes: &[i64]) -> Result<Option<Value>> {
    let mut object = match payload {
        Value::Object(object) => object,
        _ => {
            return Err(ProtocolError::Decode(
                "response is not a JSON object".into(),
            ))
        }
    };

    let result = object
        .remove("result")
        .ok_or_else(|| ProtocolError::Decode("response is missing result object".into()))?;
    let result = match result {
        Value::Object(result) => result,
        _ => {
            return Err(ProtocolError::Decode(
                "response result is not an object".into(),
            ))
        }
    };

    let code = result.get("code").and_then(as_i64).ok_or_else(|| {
        ProtocolError::Decode("response result.code is missing or not an integer".into())
    })?;
    let message = result
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("unknown error")
        .to_owned();

    if !ok_codes.contains(&code) {
        return Err(ProtocolError::Api { code, message });
    }

    Ok(object.remove("data").filter(|value| !value.is_null()))
}

/// Extract `data`, failing when it is absent.
pub fn require_data(data: Option<Value>, endpoint: &str) -> Result<Value> {
    data.ok_or_else(|| ProtocolError::Decode(format!("{endpoint}: response did not contain data")))
}

/// Extract `data` as a JSON object.
pub fn into_object(data: Option<Value>, endpoint: &str) -> Result<Map<String, Value>> {
    match require_data(data, endpoint)? {
        Value::Object(object) => Ok(object),
        _ => Err(ProtocolError::Decode(format!(
            "{endpoint}: response data is not an object"
        ))),
    }
}

/// Extract `data` as a JSON array.
pub fn into_array(data: Option<Value>, endpoint: &str) -> Result<Vec<Value>> {
    match require_data(data, endpoint)? {
        Value::Array(items) => Ok(items),
        _ => Err(ProtocolError::Decode(format!(
            "{endpoint}: response data is not an array"
        ))),
    }
}

/// Remove a list-valued field from an object.
///
/// A missing or `null` field yields an empty list; any other non-array value
/// is a decode error.
pub fn take_list(object: &mut Map<String, Value>, key: &str, endpoint: &str) -> Result<Vec<Value>> {
    match object.remove(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(ProtocolError::Decode(format!(
            "{endpoint}: data.{key} is not an array"
        ))),
    }
}

/// Read a scalar field as an owned string, accepting numbers and booleans.
pub fn string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(value_to_string)
}

/// Convert a scalar JSON value to text; structured values use JSON text.
pub fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        other => Some(other.to_string()),
    }
}

/// Decode a paged `data = { list: [...], pager: {...} }` payload.
pub fn parse_page<T>(data: Option<Value>, endpoint: &str) -> Result<Page<T>>
where
    T: DeserializeOwned + RawPayload,
{
    let mut object = into_object(data, endpoint)?;
    let raw_items = take_list(&mut object, "list", endpoint)?;
    let pager = match object.remove("pager") {
        None | Some(Value::Null) => Pager::default(),
        Some(Value::Object(pager)) => Pager::from_object(pager),
        Some(_) => {
            return Err(ProtocolError::Decode(format!(
                "{endpoint}: data.pager is not an object"
            )))
        }
    };
    let items = raw_items
        .into_iter()
        .map(|item| crate::dto::decode(item, endpoint))
        .collect::<Result<Vec<_>>>()?;
    Ok(Page { items, pager })
}

/// Accept both JSON integers and numeric strings for `result.code`.
pub(crate) fn as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ok(data: Value) -> Value {
        json!({ "result": { "code": 0, "msg": "ok" }, "data": data })
    }

    #[test]
    fn success_returns_data() {
        let data = parse_envelope(ok(json!({ "list": [] }))).unwrap();
        assert_eq!(data, Some(json!({ "list": [] })));
    }

    #[test]
    fn missing_and_null_data_both_return_none() {
        let missing = json!({ "result": { "code": 0, "msg": "ok" } });
        let null = json!({ "result": { "code": 0, "msg": "ok" }, "data": null });
        assert_eq!(parse_envelope(missing).unwrap(), None);
        assert_eq!(parse_envelope(null).unwrap(), None);
    }

    #[test]
    fn business_error_carries_code_and_message() {
        let payload = json!({ "result": { "code": -13, "msg": "重复提交" } });
        match parse_envelope(payload) {
            Err(ProtocolError::Api { code, message }) => {
                assert_eq!(code, -13);
                assert_eq!(message, "重复提交");
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn login_style_code_is_accepted_explicitly() {
        let payload = json!({ "result": { "code": -26, "msg": "ok" }, "data": { "tgt": "T" } });
        assert_eq!(
            parse_envelope_with_codes(payload, &[-26]).unwrap(),
            Some(json!({ "tgt": "T" }))
        );
    }

    #[test]
    fn malformed_envelopes_are_decode_errors() {
        for payload in [
            json!([]),
            json!({ "data": {} }),
            json!({ "result": 3 }),
            json!({ "result": { "msg": "x" } }),
        ] {
            assert!(matches!(
                parse_envelope(payload),
                Err(ProtocolError::Decode(_))
            ));
        }
        assert!(matches!(
            parse_json(b"{not json"),
            Err(ProtocolError::Decode(_))
        ));
    }

    #[test]
    fn numeric_string_code_is_accepted() {
        let payload = json!({ "result": { "code": "0", "msg": "ok" }, "data": 1 });
        assert_eq!(parse_envelope(payload).unwrap(), Some(json!(1)));
    }

    #[test]
    fn take_list_is_tolerant_of_missing_but_not_wrong_types() {
        let mut object = json!({ "ok": [], "bad": 3 }).as_object().cloned().unwrap();
        assert!(take_list(&mut object, "absent", "t").unwrap().is_empty());
        assert!(take_list(&mut object, "ok", "t").unwrap().is_empty());
        assert!(matches!(
            take_list(&mut object, "bad", "t"),
            Err(ProtocolError::Decode(_))
        ));
    }

    #[test]
    fn page_decodes_items_and_pager() {
        let data = json!({
            "list": [{ "courseId": 1, "courseName": "A" }],
            "pager": { "currentPage": 1, "pageSize": 10, "totalCount": 25, "totalPage": 3 }
        });
        let page: Page<crate::dto::Course> = parse_page(Some(data), "courses").unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id(), Some("1"));
        assert_eq!(page.pager.total_count, Some(25));
        assert!(page.pager.has_next());
    }
}
