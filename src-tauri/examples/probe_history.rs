//! Inspect corrected questions and their answer history.

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
                ("correctionStatus", json!("corrected")),
            ],
        )
        .await?
        .unwrap_or_default();
    println!("pager: {:?}", page.get("pager"));
    let list = page
        .get("list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    println!("list len: {}", list.len());
    if let Some(first) = list.first() {
        let mut summary = first.clone();
        if let Some(object) = summary.as_object_mut() {
            object.insert(
                "paperInfo".to_owned(),
                Value::String("<paperInfo>".to_owned()),
            );
        }
        println!(
            "first corrected: {}",
            serde_json::to_string_pretty(&summary)?
        );
        let question_id = first
            .get("questionId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let history_ep = Endpoint {
            name: "pm_history",
            base_url: "https://error-notebook-api.up366.cn",
            path: "/client/personalmistake/answerHistory",
            write: false,
            body: BodyKind::Json,
        };
        for body in [
            json!({"studentId": 39370562, "questionId": question_id}),
            json!({"studentId": 39370562, "questionId": question_id, "studyTaskId": "", "bookId": "", "taskId": ""}),
        ] {
            let fields: Vec<(&str, Value)> = body
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.as_str(), value.clone()))
                .collect();
            println!("=== history {body} ===");
            match client.call(&history_ep, &fields).await {
                Ok(value) => {
                    let text = value.unwrap_or_default().to_string();
                    println!("{}", &text[..text.len().min(3000)]);
                }
                Err(error) => println!("ERROR {error}"),
            }
        }
    }
    Ok(())
}
