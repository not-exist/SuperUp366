//! Verify the error-notebook correction draw/status calls.
//!
//! Logs in for this run only (env `UP366_USERNAME` / `UP366_PASSWORD`) and does
//! not touch the stored session. Read-only apart from `correction/draw`, which
//! only creates a correction batch.

use serde_json::{json, Value};
use up366_protocol::{login, BodyKind, Client, Endpoint, Session};
use up366_store::{Database, KeyringStore, SecretStore};

const ERROR_NOTEBOOK: &str = "https://error-notebook-api.up366.cn";

async fn post(
    client: &Client,
    name: &'static str,
    path: &'static str,
    body: BodyKind,
    fields: &[(&str, Value)],
) -> Result<Value, Box<dyn std::error::Error>> {
    let endpoint = Endpoint {
        name,
        base_url: ERROR_NOTEBOOK,
        path,
        write: false,
        body,
    };
    Ok(client.call(&endpoint, fields).await?.unwrap_or_default())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = std::env::var("UP366_USERNAME")?;
    let password = std::env::var("UP366_PASSWORD")?;

    let path = dirs::data_dir()
        .ok_or("no data dir")?
        .join("superup366")
        .join("up366.db");
    let db = Database::open(&path).await?;
    let accounts = db.accounts().list().await?;
    let account = accounts.first().ok_or("no account")?;
    let stored_client_id = account.client_id.clone();
    let stored: Option<Session> = KeyringStore::new()
        .load(&account.id)?
        .and_then(|text| serde_json::from_str(&text).ok());

    let client = match stored_client_id.or_else(|| stored.map(|s| s.client_id)) {
        Some(id) => Client::with_client_id(id)?,
        None => Client::new()?,
    };
    let session = login(&client, &username, &password).await?;
    println!("login ok, user_type={:?}", session.user_type);

    let user = client
        .call(up366_protocol::get("current_user").unwrap(), &[])
        .await?
        .unwrap_or_default();
    println!("current_user: {}", serde_json::to_string_pretty(&user)?);
    let uid = user.get("uid").and_then(Value::as_i64).unwrap_or_default();
    let organ = user
        .get("organId")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let stage = user
        .get("schoolType")
        .and_then(Value::as_i64)
        .unwrap_or_default();

    let overview = post(
        &client,
        "pm_overview",
        "/client/personalmistake/overview",
        BodyKind::Json,
        &[
            ("studentId", json!(uid)),
            ("organId", json!(organ)),
            ("stageId", json!(stage)),
        ],
    )
    .await?;
    let groups = overview
        .get("groupList")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    println!(
        "overview: remaining={} corrected={} today={} groups={}",
        overview.get("remainingCount").unwrap_or(&Value::Null),
        overview.get("correctedCount").unwrap_or(&Value::Null),
        overview.get("todayCorrectedCount").unwrap_or(&Value::Null),
        groups.len()
    );
    let group = groups
        .iter()
        .find(|group| {
            group
                .get("remainingCount")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                > 0
        })
        .cloned();
    let group_key = group
        .as_ref()
        .and_then(|group| group.get("groupKey"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let remaining = group
        .as_ref()
        .and_then(|group| group.get("remainingCount"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    println!("group_key={group_key:?} remaining={remaining}");

    let count = remaining.clamp(1, 5);
    let mut form: Vec<(&str, Value)> =
        vec![("studentId", json!(uid)), ("questionCount", json!(count))];
    if !group_key.is_empty() {
        form.push(("groupKey", json!(group_key)));
    }
    let draw = post(
        &client,
        "pm_draw",
        "/client/personalmistake/correction/draw",
        BodyKind::Form,
        &form,
    )
    .await?;
    let mut summary = draw.clone();
    if let Some(object) = summary.as_object_mut() {
        for value in object.values_mut() {
            if value.as_str().map(str::len).unwrap_or(0) > 200 {
                *value = Value::String(format!("<string {} bytes>", value.as_str().unwrap().len()));
            }
        }
    }
    println!(
        "draw keys={:?}",
        draw.as_object()
            .map(|object| object.keys().cloned().collect::<Vec<_>>())
    );
    println!("{}", serde_json::to_string_pretty(&summary)?);
    for key in ["paperData", "answerData"] {
        if let Some(text) = draw.get(key).and_then(Value::as_str) {
            println!("--- {key} ---\n{text}");
        }
    }

    let batch_id = draw
        .get("batchId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if !batch_id.is_empty() {
        let status = post(
            &client,
            "pm_correction_status",
            "/client/personalmistake/correction/status",
            BodyKind::Form,
            &[
                ("batchId", json!(batch_id)),
                ("uid", json!(uid.to_string())),
            ],
        )
        .await?;
        println!(
            "status keys={:?} status={:?}",
            status
                .as_object()
                .map(|object| object.keys().cloned().collect::<Vec<_>>()),
            status.get("status")
        );
    }
    Ok(())
}
