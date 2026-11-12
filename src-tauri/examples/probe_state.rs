//! Write a Playwright storage-state file from the app's stored session.

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

    let cookies: Vec<serde_json::Value> = session
        .cookies
        .iter()
        .map(|(name, value)| {
            json!({
                "name": name,
                "value": value,
                "domain": ".up366.cn",
                "path": "/",
                "expires": -1,
                "httpOnly": false,
                "secure": true,
                "sameSite": "Lax"
            })
        })
        .collect();
    let state = json!({ "cookies": cookies, "origins": [] });
    std::fs::write(
        "H:/project/up366/.playwright-state.json",
        serde_json::to_string(&state)?,
    )?;
    println!("cookies written: {}", session.cookies.len());
    Ok(())
}
