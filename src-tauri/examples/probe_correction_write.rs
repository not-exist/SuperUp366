//! One real correction submission: draw one question, submit it, poll status.
//!
//! Credentials come from `UP366_USERNAME` / `UP366_PASSWORD`; nothing is saved.

use serde_json::Value;
use up366_protocol::adapters::{
    build_tasks_json, correction_draw, correction_overview, correction_status, submit_correction,
    CorrectionContext,
};
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
    let organ = user
        .get("organId")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let stage = user
        .get("schoolType")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    println!("uid={uid} organ={organ} stage={stage}");

    let before = correction_overview(&client, uid, organ, stage).await?;
    println!(
        "overview before: remaining={} corrected={} today={}",
        before.remaining_count, before.corrected_count, before.today_corrected_count
    );

    let wanted = std::env::var("UP366_QTYPE").unwrap_or_default();
    let group = before.group_list.iter().find(|group| {
        group.remaining_count > 0
            && (wanted.is_empty() || group.question_type_name.contains(&wanted))
    });
    let group_key = group.map(|group| group.group_key.as_str());
    println!(
        "group: {:?} remaining={:?}",
        group.map(|group| group.question_type_name.as_str()),
        group.map(|group| group.remaining_count)
    );

    let paper = correction_draw(&client, uid, &[], 1, group_key)
        .await?
        .ok_or("nothing left to correct")?;
    let question = paper.questions.first().ok_or("empty paper")?;
    println!(
        "drawn: batch={} question={} type={} score={} stem={} answer={}",
        paper.batch_id,
        question.question_id,
        question.question_type,
        question.score,
        question.stem,
        paper.answer_text(&question.question_id)
    );
    println!(
        "qstJson rows: {}",
        up366_protocol::adapters::qst_json(&paper)
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
    );
    println!("markdatao: {}", up366_protocol::adapters::mark_xml(&paper));

    let study_date = up366_protocol::adapters::server_time(&client)
        .await
        .unwrap_or_else(|_| up366_store::now_millis());
    let context = CorrectionContext {
        uid: uid.to_string(),
        client_id: client.client_id().to_owned(),
        course_id: -1,
        study_task_id: String::new(),
    };
    let tasks = build_tasks_json(&paper, &context, 2, study_date, "PRODUCT");
    println!("submitting...");
    match submit_correction(&client, &tasks, client.now_ms()).await {
        Ok(value) => println!("submit ok: {value}"),
        Err(error) => {
            println!("submit FAILED: {error:?}");
            return Ok(());
        }
    }

    for attempt in 0..6 {
        let status = correction_status(&client, &paper.batch_id, &uid.to_string()).await?;
        println!(
            "status[{attempt}]: status={} percent={} submitDate={}",
            status.status, status.percent, status.submit_date
        );
        if status.status == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    let after = correction_overview(&client, uid, organ, stage).await?;
    println!(
        "overview after: remaining={} corrected={} today={}",
        after.remaining_count, after.corrected_count, after.today_corrected_count
    );
    Ok(())
}
