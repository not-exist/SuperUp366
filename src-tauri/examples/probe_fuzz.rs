//! Fuzz candidate write route names on error-notebook-api with an empty JSON body.

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

    let names = [
        "submit",
        "correct",
        "correction",
        "answer",
        "revise",
        "finish",
        "kill",
        "clean",
        "remove",
        "delete",
        "mark",
        "record",
        "complete",
        "save",
        "update",
        "status",
        "markCorrected",
        "markcorrect",
        "correctQuestion",
        "batchCorrect",
        "correctBatch",
        "submitAnswer",
        "submitCorrection",
        "correctionSubmit",
        "doCorrect",
        "doCorrection",
        "finishCorrect",
        "finishCorrection",
        "saveCorrection",
        "updateStatus",
        "setCorrected",
        "batchUpdate",
        "batchSubmit",
        "handle",
        "handleCorrect",
        "review",
        "reviewed",
        "practice",
        "exercise",
        "result",
        "results",
        "sync",
        "import",
        "confirm",
        "commit",
        "apply",
        "change",
        "modify",
    ];
    for name in names {
        let path = format!("/client/personalmistake/{name}");
        let endpoint = Endpoint {
            name: "pm_fuzz",
            base_url: "https://error-notebook-api.up366.cn",
            path: Box::leak(path.clone().into_boxed_str()),
            write: true,
            body: BodyKind::Json,
        };
        match client.call(&endpoint, &[]).await {
            Ok(value) => println!("{name} -> OK {:?}", value.map(|v| v.to_string())),
            Err(error) => {
                let text = error.to_string();
                if !text.contains("no_auth_zuul") {
                    println!("{name} -> {text}");
                }
            }
        }
    }
    Ok(())
}
