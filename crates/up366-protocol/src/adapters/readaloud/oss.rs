//! OSS uploads for recordings.
//!
//! Two confirmed facts drive this module:
//!
//! 1. Temporary credentials come from
//!    `GET /front/aliyunoss/token/v2?organId={schoolId}`. The response `data`
//!    is a list of `{biz, ossBucketInfo}` entries; recordings use `biz=up366`.
//! 2. The transfer is a signed `PUT` (Aliyun OSS V1 signature), not a
//!    multipart upload. The signature must include the STS
//!    `x-oss-security-token` header and a **base64** `Content-MD5`; omitting
//!    either makes OSS answer `403`.

use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, DATE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha1::Sha1;

use crate::client::Client;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};

const CONTENT_TYPE_VALUE: &str = "audio/mpeg";
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

type HmacSha1 = Hmac<Sha1>;

/// OSS upload credentials for one recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadTicket {
    /// Bucket name from the STS response.
    pub bucket: String,
    /// Bucket endpoint host from the STS response.
    pub endpoint: String,
    /// Access key id.
    pub access_key_id: String,
    /// Access key secret.
    pub access_key_secret: String,
    /// Security token for the temporary credentials.
    pub security_token: String,
    /// Object name to upload to.
    pub object_name: String,
}

impl UploadTicket {
    /// Object URL, composed the way the client composes it:
    /// `https://{bucketname}.{defaulthostid}/{objectName}`.
    pub fn url(&self) -> String {
        format!(
            "https://{}.{}/{}",
            self.bucket, self.endpoint, self.object_name
        )
    }
}

/// Fetch OSS upload credentials for one object.
///
/// `organ_id` is the school id, required by the endpoint. The response is a
/// list of buckets; the student bucket is the `biz = up366` entry.
pub async fn upload_ticket(
    client: &Client,
    organ_id: &str,
    object_name: &str,
) -> Result<UploadTicket> {
    let endpoint = get("aliyun_oss_token")
        .ok_or_else(|| ProtocolError::Decode("missing endpoint: aliyun_oss_token".into()))?;
    let data = client
        .call(endpoint, &[("organId", json!(organ_id))])
        .await?;
    let info = bucket_info(data.as_ref())?;
    Ok(UploadTicket {
        bucket: field(info, "bucketname")?,
        endpoint: field(info, "defaulthostid")?,
        access_key_id: field(info, "tmpAkId")?,
        access_key_secret: field(info, "tmpAkSecret")?,
        security_token: field(info, "securityToken")?,
        object_name: object_name.to_owned(),
    })
}

/// Pick the student bucket's `ossBucketInfo` object out of the STS payload.
fn bucket_info(data: Option<&Value>) -> Result<&Map<String, Value>> {
    let entries: Vec<&Value> = match data {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(Value::Object(object)) if object.contains_key("ossBucketInfo") => vec![data.unwrap()],
        Some(Value::Object(object)) => object.values().collect(),
        _ => Vec::new(),
    };
    let chosen = entries
        .iter()
        .find(|entry| entry.get("biz").and_then(Value::as_str) == Some("up366"))
        .or_else(|| {
            entries
                .iter()
                .find(|entry| entry.get("ossBucketInfo").is_some())
        })
        .ok_or_else(|| {
            ProtocolError::Decode("oss token response did not contain ossBucketInfo".to_owned())
        })?;
    chosen
        .get("ossBucketInfo")
        .and_then(Value::as_object)
        .ok_or_else(|| ProtocolError::Decode("ossBucketInfo is not an object".to_owned()))
}

fn field(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| ProtocolError::Decode(format!("oss token response missing {key}")))
}

/// Upload `bytes` with a V1 signed PUT.
///
/// The signature covers the fixed headers plus the security token; see
/// `signature_for` for the exact string.
pub async fn upload_object(ticket: &UploadTicket, bytes: Vec<u8>) -> Result<()> {
    if bytes.is_empty() {
        return Err(ProtocolError::Invalid("recording is empty".to_owned()));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(ProtocolError::Invalid(format!(
            "recording is larger than {} MiB",
            MAX_UPLOAD_BYTES / 1024 / 1024
        )));
    }

    let date = Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let (content_md5, signature) = signature_for(ticket, &bytes, &date);
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("OSS {}:{}", ticket.access_key_id, signature))
            .map_err(|error| ProtocolError::Invalid(error.to_string()))?,
    );
    headers.insert(
        DATE,
        HeaderValue::from_str(&date).map_err(|error| ProtocolError::Invalid(error.to_string()))?,
    );
    headers.insert(
        HeaderName::from_static("content-md5"),
        HeaderValue::from_str(&content_md5)
            .map_err(|error| ProtocolError::Invalid(error.to_string()))?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE));
    headers.insert(
        "x-oss-security-token",
        HeaderValue::from_str(&ticket.security_token)
            .map_err(|error| ProtocolError::Invalid(error.to_string()))?,
    );

    let response = reqwest::Client::new()
        .put(ticket.url())
        .headers(headers)
        .timeout(UPLOAD_TIMEOUT)
        .body(bytes)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(ProtocolError::Http {
            status: status.as_u16(),
            body: truncate(&body, 300),
        });
    }
    Ok(())
}

/// `(Content-MD5 base64, Authorization signature)` for one upload.
fn signature_for(ticket: &UploadTicket, bytes: &[u8], date: &str) -> (String, String) {
    let content_md5 = BASE64.encode(md5_digest(bytes));
    let resource = format!("/{}/{}", ticket.bucket, ticket.object_name);
    let string_to_sign = format!(
        "PUT\n{content_md5}\n{CONTENT_TYPE_VALUE}\n{date}\n\
         x-oss-security-token:{}\n{resource}",
        ticket.security_token
    );
    let mut mac = HmacSha1::new_from_slice(ticket.access_key_secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(string_to_sign.as_bytes());
    let signature = BASE64.encode(mac.finalize().into_bytes());
    (content_md5, signature)
}

fn md5_digest(bytes: &[u8]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let digest = Md5::digest(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    fn ticket() -> UploadTicket {
        UploadTicket {
            bucket: "up366-bucket1".to_owned(),
            endpoint: "oss-cn-beijing.aliyuncs.com".to_owned(),
            access_key_id: "STS.test".to_owned(),
            access_key_secret: "secret".to_owned(),
            security_token: "token".to_owned(),
            object_name: "storage/102/study/a/2026/11/09/u1/q1.mp3".to_owned(),
        }
    }

    #[test]
    fn the_upload_url_is_bucket_dot_endpoint_over_the_object_name() {
        assert_eq!(
            ticket().url(),
            "https://up366-bucket1.oss-cn-beijing.aliyuncs.com/storage/102/study/a/2026/11/09/u1/q1.mp3"
        );
    }

    #[test]
    fn the_signature_covers_the_security_token_and_base64_md5() {
        let bytes = b"hello mp3";
        let (md5, signature) = signature_for(&ticket(), bytes, "Mon, 09 Nov 2026 04:10:26 GMT");
        assert_eq!(md5, BASE64.encode(md5_digest(bytes)));
        assert!(signature.len() > 20);

        // Same input, same signature (HMAC-SHA1 is deterministic).
        let (_, again) = signature_for(&ticket(), bytes, "Mon, 09 Nov 2026 04:10:26 GMT");
        assert_eq!(signature, again);
    }

    #[tokio::test]
    async fn the_student_bucket_is_selected_from_the_list() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/front/aliyunoss/token/v2")
                .query_param("organId", "3105");
            then.status(200).json_body(ok(json!([
                {
                    "biz": "video-uncheck",
                    "ossBucketInfo": { "bucketname": "voice", "defaulthostid": "oss-accelerate.aliyuncs.com", "tmpAkId": "V", "tmpAkSecret": "V", "securityToken": "V" }
                },
                {
                    "biz": "up366",
                    "ossBucketInfo": { "bucketname": "up366-bucket1", "defaulthostid": "oss-cn-beijing.aliyuncs.com", "tmpAkId": "A", "tmpAkSecret": "S", "securityToken": "T" }
                }
            ])));
        });

        let ticket = upload_ticket(&client(&server), "3105", "storage/1/a.mp3")
            .await
            .unwrap();
        assert_eq!(ticket.bucket, "up366-bucket1");
        assert_eq!(ticket.access_key_id, "A");
        assert_eq!(ticket.object_name, "storage/1/a.mp3");
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn a_response_without_a_bucket_is_a_decode_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/front/aliyunoss/token/v2");
            then.status(200).json_body(ok(json!({ "biz": "up366" })));
        });
        assert!(matches!(
            upload_ticket(&client(&server), "3105", "a.mp3").await,
            Err(ProtocolError::Decode(_))
        ));
    }
}
