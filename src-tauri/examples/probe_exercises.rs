//! Read-only probe for /client/personalmistake/exercises.

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
                ("pageSize", json!(5)),
                ("studentId", json!(39370562)),
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
    println!("question_id={question_id}");

    let exercises_ep = Endpoint {
        name: "pm_exercises",
        base_url: "https://error-notebook-api.up366.cn",
        path: "/client/personalmistake/exercises",
        write: false,
        body: BodyKind::Json,
    };
    for body in [
        json!({"studentId": 39370562, "questionIdList": [question_id], "questionCount": 1, "mistakeSourceType": "exercise"}),
        json!({"studentId": 39370562, "questionIdList": [question_id], "questionCount": 1, "mistakeSourceType": "selfPractice"}),
        json!({"studentId": 39370562, "questionIdList": [question_id], "questionCount": 1, "mistakeSourceType": "all"}),
    ] {
        let fields: Vec<(&str, Value)> = body
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.as_str(), value.clone()))
            .collect();
        println!("=== {body} ===");
        match client.call(&exercises_ep, &fields).await {
            Ok(value) => {
                let value = value.unwrap_or_default();
                let text = value.to_string();
                println!("{}", &text[..text.len().min(3000)]);
            }
            Err(error) => println!("ERROR {error}"),
        }
    }
    Ok(())
}
