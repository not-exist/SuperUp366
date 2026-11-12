//! Inspect /client/personalmistake/correction/draw.

use serde_json::{json, Value};
use up366_protocol::{BodyKind, Client, Endpoint, Session};
use up366_store::{Database, KeyringStore, SecretStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = dirs::data_dir()
        .ok_or("no data dir")?
        .join("superup366")
        .join("up366.db");
    let db = Database::open(&path).await?;
    let accounts = db.accounts().list().await?;
    let account = accounts.first().ok_or("no account")?;
    let secrets = KeyringStore::new();
    let json_text = secrets.load(&account.id)?.ok_or("no session")?;
    let session: Session = serde_json::from_str(&json_text)?;
    let client = Client::with_session(session)?;

    let page_ep = Endpoint {
        name: "pm_page",
        base_url: "https://error-notebook-api.up366.cn",
        path: "/client/personalmistake/page",
        write: false,
        body: BodyKind::Json,
    };
    let page = client
        .call(
            &page_ep,
            &[
                ("currentPage", json!(1)),
                ("pageSize", json!(1)),
                ("studentId", json!(39370562)),
                ("correctionStatus", json!("uncorrected")),
            ],
        )
        .await?
        .unwrap_or_default();
    let first = page
        .get("list")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .cloned()
        .unwrap_or_default();
    let question_id = first
        .get("questionId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let sub_ids: Vec<String> = first
        .get("questionStatsList")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| item.get("questionId").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    println!("parent={question_id} sub_ids={sub_ids:?}");

    let draw_ep = Endpoint {
        name: "pm_draw",
        base_url: "https://error-notebook-api.up366.cn",
        path: "/client/personalmistake/correction/draw",
        write: false,
        body: BodyKind::Json,
    };
    let draw = match client
        .call(
            &draw_ep,
            &[
                ("studentId", json!(39370562)),
                (
                    "questionIdList",
                    json!(if sub_ids.is_empty() {
                        vec![question_id.to_owned()]
                    } else {
                        sub_ids.clone()
                    }),
                ),
                (
                    "questionCount",
                    json!(if sub_ids.is_empty() { 1 } else { sub_ids.len() }),
                ),
            ],
        )
        .await
    {
        Ok(value) => value.unwrap_or_default(),
        Err(error) => {
            println!("draw error: {error:?}");
            return Ok(());
        }
    };
    let text = draw.to_string();
    println!(
        "draw top keys: {:?}",
        draw.as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
    );
    println!("draw size: {}", text.len());
    if let Some(object) = draw.as_object() {
        for (key, value) in object {
            let summary = match value {
                Value::String(s) if s.len() > 80 => format!("<string {} bytes>", s.len()),
                other => other.to_string(),
            };
            println!("- {key}: {}", &summary[..summary.len().min(300)]);
        }
    }
    Ok(())
}
