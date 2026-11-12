//! Read-only: draw one correction question and print both XML documents.

use serde_json::Value;
use up366_protocol::adapters::correction_draw;
use up366_protocol::{login, Client};
use up366_store::Database;

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
    let client = match account.client_id.clone() {
        Some(id) => Client::with_client_id(id)?,
        None => Client::new()?,
    };
    login(&client, &username, &password).await?;
    let user = client
        .call(up366_protocol::get("current_user").unwrap(), &[])
        .await?
        .unwrap_or_default();
    let uid = user.get("uid").and_then(Value::as_i64).unwrap_or_default();

    let overview = client
        .call(
            up366_protocol::get("pm_overview").unwrap(),
            &[
                ("studentId", serde_json::json!(uid)),
                (
                    "organId",
                    serde_json::json!(user
                        .get("organId")
                        .and_then(Value::as_i64)
                        .unwrap_or_default()),
                ),
                (
                    "stageId",
                    serde_json::json!(user
                        .get("schoolType")
                        .and_then(Value::as_i64)
                        .unwrap_or_default()),
                ),
            ],
        )
        .await?
        .unwrap_or_default();
    let groups = overview
        .get("groupList")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for group in groups.iter().take(3) {
        println!("GROUP {}", serde_json::to_string(group)?);
    }

    let paper = correction_draw(&client, uid, &[], 1, None)
        .await?
        .ok_or("nothing left")?;
    println!("=== paperData ===\n{}", paper.paper_xml);
    println!("=== answerData ===\n{}", paper.answer_xml);
    Ok(())
}
