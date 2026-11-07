//! Read-aloud adapters: the slot list, OSS credentials and the round preview.
//!
//! What is confirmed (see `docs/protocol/to-probe.md` §4, sourced from the
//! archived live-payload analysis on `main`):
//!
//! - one recording per **question**;
//! - audio is uploaded to OSS first and referenced by URL in
//!   `<answer_url>` (paper XML) and `qstJson[].elementAttr.record_url`;
//! - `objectName` = `storage{userType}/study/a/{yyyy/mm/dd}/{uid}/{file}`;
//! - credentials come from `/front/aliyunoss/token/v2?organId={schoolId}` and
//!   the transfer is a multipart upload.
//!
//! What is *not* confirmed: which submit branch a homework task uses, the exact
//! `mark_datao` numbering, and whether the server re-grades asynchronously.
//! [`submit_readaloud`] therefore exists but refuses to send, and only
//! [`upload_recording`] performs network writes. A dry run is the default.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client::Client;
use crate::codec;
use crate::endpoint::get;
use crate::error::{ProtocolError, Result};

/// One slot that needs an uploaded recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadAloudSlot {
    pub question_id: String,
    pub number: usize,
    pub prompt: String,
    pub sentence: String,
    /// Reference audio supplied by the server, if any.
    pub audio_url: Option<String>,
    /// Set once a recording has been uploaded.
    pub record_url: Option<String>,
}

/// The slots of one read-aloud task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadAloudTaskView {
    pub job_id: String,
    pub title: String,
    pub submitted: bool,
    pub slots: Vec<ReadAloudSlot>,
}

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

/// Parameters for reading a task's paper.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperQuery {
    pub paper_id: String,
}

/// Where a recording's bytes came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    /// Object name the bytes will be stored under.
    pub object_name: String,
    /// Length of the payload, used for the multipart part size.
    pub size: u64,
    /// Raw audio bytes.
    pub bytes: Vec<u8>,
}

/// Fetch OSS upload credentials for one object.
///
/// `organ_id` is the school id, required by the endpoint. The values are
/// deliberately read field by field so a shape change is an error rather than
/// a silent empty credential.
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
    let object = codec::into_object(data, endpoint.name)?;
    // The credentials live under `ossBucketInfo`, keyed by bucket id upstream.
    let bucket_info = nested_object(&object, "ossBucketInfo")?;
    Ok(UploadTicket {
        bucket: nested_field(bucket_info, "bucketname")?,
        endpoint: nested_field(bucket_info, "defaulthostid")?,
        access_key_id: nested_field(bucket_info, "tmpAkId")?,
        access_key_secret: nested_field(bucket_info, "tmpAkSecret")?,
        security_token: nested_field(bucket_info, "securityToken")?,
        object_name: object_name.to_owned(),
    })
}

/// Descend into one nested object.
fn nested_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    object
        .get(key)
        .and_then(|value| value.as_object())
        .ok_or_else(|| ProtocolError::Decode(format!("oss token response missing {key}")))
}

/// Read a required string field, failing with a precise message.
fn nested_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| ProtocolError::Decode(format!("oss token response missing {key}")))
}

/// Build the payload for one read-aloud round **without sending it**.
///
/// This is what the confirmation dialog renders. It is deliberately the same
/// function the submit path would use, so the preview cannot drift from the
/// request.
pub fn preview_submission(task: &ReadAloudTaskView) -> Result<Value> {
    if task.slots.iter().any(|slot| slot.record_url.is_none()) {
        return Err(ProtocolError::Invalid(
            "every read-aloud slot must have an uploaded recording".to_owned(),
        ));
    }
    Ok(json!({
        "jobId": task.job_id,
        "title": task.title,
        "slots": task.slots,
    }))
}

/// Submit one read-aloud round.
///
/// Refuses to send while the branch, `mark_datao` numbering and `qstJson`
/// shape remain unverified (see `docs/protocol/to-probe.md` §4). The preview is
/// still produced so the UI can show exactly what would go out.
pub async fn submit_readaloud(
    client: &Client,
    task: &ReadAloudTaskView,
    dry_run: bool,
) -> Result<Value> {
    let payload = preview_submission(task)?;
    if dry_run {
        return Ok(payload);
    }
    let _ = client;
    Err(ProtocolError::Unsupported(
        "read-aloud submission is not enabled until the payload shape is \
         confirmed against a real task (see docs/protocol/to-probe.md §4)"
            .to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;

    fn task() -> ReadAloudTaskView {
        ReadAloudTaskView {
            job_id: "JOB-1".to_owned(),
            title: "Unit 4 跟读".to_owned(),
            submitted: false,
            slots: vec![ReadAloudSlot {
                question_id: "Q1".to_owned(),
                number: 1,
                prompt: "Hello world.".to_owned(),
                sentence: "Hello world.".to_owned(),
                audio_url: None,
                record_url: Some("https://oss/a.mp3".to_owned()),
            }],
        }
    }

    #[test]
    fn a_slot_without_a_recording_is_rejected_before_any_request() {
        let mut task = task();
        task.slots[0].record_url = None;
        assert!(preview_submission(&task).is_err());
    }

    #[tokio::test]
    async fn submit_refuses_until_the_shape_is_confirmed() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/task/score/submit/v2");
            then.status(200).json_body(ok(json!({})));
        });

        let error = submit_readaloud(&client(&server), &task(), false)
            .await
            .unwrap_err();
        assert!(matches!(error, ProtocolError::Unsupported(_)));
        mock.assert_hits(0);
    }

    #[test]
    fn the_upload_url_is_bucket_dot_endpoint_over_the_object_name() {
        let ticket = UploadTicket {
            bucket: "up366".to_owned(),
            endpoint: "oss-cn-hangzhou.aliyuncs.com".to_owned(),
            access_key_id: "k".to_owned(),
            access_key_secret: "s".to_owned(),
            security_token: "t".to_owned(),
            object_name: "storage102/study/a/2026/11/07/u1/q1.mp3".to_owned(),
        };
        assert_eq!(
            ticket.url(),
            "https://up366.oss-cn-hangzhou.aliyuncs.com/storage102/study/a/2026/11/07/u1/q1.mp3"
        );
    }

    #[test]
    fn a_dry_run_returns_the_preview_and_never_sends() {
        // The score endpoint is not available in a sync test, and that is the
        // point: a dry run must not need a client at all.
        let value = preview_submission(&task()).unwrap();
        assert_eq!(value["jobId"], "JOB-1");
        assert_eq!(value["slots"].as_array().unwrap().len(), 1);
    }
}
