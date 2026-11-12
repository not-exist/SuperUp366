//! Try correction/draw with the source task context.

use serde_json::{json, Value};
use up366_protocol::{BodyKind, Client, Endpoint, Session};
use up366_store::{Database, KeyringStore, SecretStore};

async fn post(
    client: &Client,
    path: &'static str,
    body: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let endpoint = Endpoint {
        name: "pm",
        base_url: "https://error-notebook-api.up366.cn",
        path,
        write: false,
        body: BodyKind::Json,
    };
    let fields: Vec<(&str, Value)> = body
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, value)| (key.as_str(), value.clone()))
        .collect();
    Ok(client.call(&endpoint, &fields).await?.unwrap_or_default())
}

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

    let overview = post(
        &client,
        "/client/personalmistake/overview",
        json!({
            "studentId": 39370562, "organId": 1001, "stageId": 3
        }),
    )
    .await?;
    println!(
        "overview keys={:?}",
        overview
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
    );
    let groups = overview
        .get("groupList")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(group) = groups
        .iter()
        .find(|g| g.get("remainingCount").and_then(Value::as_i64).unwrap_or(0) == 1)
    {
        let group_key = group
            .get("groupKey")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let count = group
            .get("remainingCount")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        println!("group_key={group_key} count={count}");
        for body in [
            json!({"studentId": 39370562, "questionIdList": [], "questionCount": count, "groupKey": group_key}),
            json!({"studentId": 39370562, "questionCount": count, "groupKey": group_key}),
        ] {
            println!("=== draw group ===");
            match post(&client, "/client/personalmistake/correction/draw", body).await {
                Ok(value) => println!(
                    "keys={:?} size={}",
                    value
                        .as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>()),
                    value.to_string().len()
                ),
                Err(error) => println!("ERROR {error}"),
            }
        }
    } else {
        println!("no group with remaining");
    }

    let page = post(&client, "/client/personalmistake/page", json!({
        "currentPage": 1, "pageSize": 1, "studentId": 39370562, "correctionStatus": "uncorrected"
    })).await?;
    let first = page
        .get("list")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .cloned()
        .unwrap_or_default();
    let parent = first
        .get("questionId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let sub_ids: Vec<String> = first
        .get("questionStatsList")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| item.get("questionId").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    println!("parent={parent} sub_ids={sub_ids:?}");

    let exercises = post(
        &client,
        "/client/personalmistake/exercises",
        json!({
            "studentId": 39370562,
            "questionIdList": [parent],
            "questionCount": 1,
            "mistakeSourceType": "exercise"
        }),
    )
    .await?;
    let source = exercises
        .as_array()
        .and_then(|list| list.first())
        .cloned()
        .unwrap_or_default();
    let course_id = source.get("courseId").cloned().unwrap_or(Value::Null);
    let study_task_id = source
        .get("exerciseList")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(|item| item.get("studyTaskId"))
        .cloned()
        .unwrap_or(Value::Null);
    println!("courseId={course_id} studyTaskId={study_task_id}");

    for ids in [vec![parent.clone()], sub_ids.clone()] {
        for with_source in [false, true] {
            let mut body = json!({
                "studentId": 39370562,
                "questionIdList": ids,
                "questionCount": ids.len()
            });
            if with_source {
                body["courseId"] = course_id.clone();
                body["studyTaskId"] = study_task_id.clone();
                body["stageId"] = json!("3");
                body["subjectId"] = json!("303");
            }
            println!("=== draw ids={:?} with_source={with_source} ===", ids);
            match post(&client, "/client/personalmistake/correction/draw", body).await {
                Ok(value) => {
                    println!(
                        "keys={:?} size={}",
                        value
                            .as_object()
                            .map(|o| o.keys().cloned().collect::<Vec<_>>()),
                        value.to_string().len()
                    );
                    let mut summary = value.clone();
                    if let Some(object) = summary.as_object_mut() {
                        for key in ["paperInfo", "paperData", "answerData", "userAnswerData"] {
                            if object.contains_key(key) {
                                object.insert(key.to_owned(), Value::String(format!("<{}>", key)));
                            }
                        }
                    }
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&summary)?
                            .chars()
                            .take(1500)
                            .collect::<String>()
                    );
                }
                Err(error) => println!("ERROR {error}"),
            }
        }
    }
    Ok(())
}
