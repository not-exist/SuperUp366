//! Read-only probe for the new error-notebook API.

use serde_json::json;
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

    let user = client
        .call(up366_protocol::get("current_user").unwrap(), &[])
        .await?;
    println!("current_user: {}", serde_json::to_string_pretty(&user)?);
    let data = user.as_ref().and_then(|value| value.as_object()).unwrap();
    let uid = data
        .get("uid")
        .map(|v| match v {
            serde_json::Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let organ = data
        .get("organId")
        .map(|v| match v {
            serde_json::Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let school_type = data
        .get("schoolType")
        .map(|v| match v {
            serde_json::Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    println!("uid={uid} organId={organ} schoolType={school_type}");

    let overview = Endpoint {
        name: "pm_overview",
        base_url: "https://error-notebook-api.up366.cn",
        path: "/client/personalmistake/overview",
        write: false,
        body: BodyKind::Json,
    };
    let value = client
        .call(
            &overview,
            &[
                ("studentId", json!(uid)),
                ("organId", json!(organ)),
                ("stageId", json!(school_type)),
            ],
        )
        .await?;
    println!("overview: {}", serde_json::to_string_pretty(&value)?);

    Ok(())
}
