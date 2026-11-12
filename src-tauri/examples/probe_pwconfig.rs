//! Write a Playwright config carrying the session headers.

use serde_json::json;
use up366_protocol::Session;
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

    let config = json!({
        "use": {
            "extraHTTPHeaders": {
                "u3t": session.tgt,
                "clientId": session.client_id,
                "X-Requested-With": "XMLHttpRequest"
            }
        }
    });
    std::fs::write(
        "H:/project/up366/.pw-config.json",
        serde_json::to_string(&config)?,
    )?;
    println!("config written");
    Ok(())
}
