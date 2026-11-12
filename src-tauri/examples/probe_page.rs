//! Summarize one /client/personalmistake/page response.

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

    let endpoint = Endpoint {
        name: "pm_page",
        base_url: "https://error-notebook-api.up366.cn",
        path: "/client/personalmistake/page",
        write: false,
        body: BodyKind::Json,
    };
    let value = client
        .call(
            &endpoint,
            &[
                ("currentPage", json!(1)),
                ("pageSize", json!(20)),
                ("studentId", json!(39370562)),
            ],
        )
        .await?
        .unwrap_or_default();

    println!(
        "top keys: {:?}",
        value
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
    );
    println!("pager: {:?}", value.get("pager"));
    let list = value
        .get("list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    println!("list len: {}", list.len());
    if let Some(first) = list.first() {
        println!(
            "item keys: {:?}",
            first
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
        );
        let mut summary = first.clone();
        if let Some(object) = summary.as_object_mut() {
            for key in [
                "paperInfo",
                "userAnswerData",
                "paperData",
                "answerData",
                "analysisData",
            ] {
                if object.contains_key(key) {
                    object.insert(key.to_owned(), Value::String(format!("<{}>", key)));
                }
            }
        }
        println!("first item: {}", serde_json::to_string_pretty(&summary)?);
    }
    Ok(())
}
