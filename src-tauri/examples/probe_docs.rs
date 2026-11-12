//! Try the service's OpenAPI document with the real session.

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

    for path in [
        "/v3/api-docs",
        "/v3/api-docs/swagger-config",
        "/client/v3/api-docs",
        "/personalmistake/v3/api-docs",
    ] {
        let endpoint = Endpoint {
            name: "docs",
            base_url: "https://error-notebook-api.up366.cn",
            path,
            write: false,
            body: BodyKind::Query,
        };
        match client.call(&endpoint, &[]).await {
            Ok(value) => {
                let text = value.map(|v| v.to_string()).unwrap_or_default();
                println!("=== {path} === {} bytes", text.len());
                println!("{}", &text[..text.len().min(600)]);
            }
            Err(error) => println!("=== {path} === ERROR {error}"),
        }
    }
    Ok(())
}
