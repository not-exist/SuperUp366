//! Inspect the generic answer book (通用作答).

use serde_json::json;
use up366_protocol::{Client, Session};
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

    let value = client
        .call(
            up366_protocol::get("book_info").unwrap(),
            &[
                ("bookId", json!("5DCDFABF59D854C9B01C9811B08548D0")),
                ("bookType", json!(1)),
                ("originalBookId", json!("")),
            ],
        )
        .await?;
    let value = value.unwrap_or_default();
    let text = serde_json::to_string_pretty(&value)?;
    println!("{}", &text[..text.len().min(6000)]);
    Ok(())
}
