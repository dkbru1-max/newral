use tokio_postgres::{Client, Error, GenericClient};

use crate::models::TaskPayload;

const SQL_INSERT_FLAG: &str =
    "INSERT INTO flags (device_id, task_id, reason, details) VALUES ($1, $2, $3, $4)";
const SQL_UPSERT_REPUTATION: &str = "INSERT INTO device_reputation (device_id, score) \
VALUES ($1, $2) \
ON CONFLICT (device_id) DO UPDATE SET score = device_reputation.score + $2, updated_at = NOW() \
RETURNING score";
const SQL_SELECT_PROJECT: &str = "SELECT id, name FROM projects WHERE id = $1";
const SQL_PROJECT_SCHEMA_NAME: &str = "SELECT project_schema_name($1, $2)";

fn task_payload_select_sql(schema: &str) -> String {
    format!("SELECT payload FROM {}.tasks WHERE id = $1", schema)
}

fn task_results_latest_sql(schema: &str) -> String {
    format!(
        "SELECT result FROM {}.task_results WHERE task_id = $1 ORDER BY id DESC LIMIT 1",
        schema
    )
}

fn task_group_count_sql(schema: &str) -> String {
    format!(
        "SELECT status, COUNT(*) AS count FROM {}.tasks WHERE payload->>'group_id' = $1 GROUP BY status",
        schema
    )
}

pub async fn resolve_project_schema(db: &mut Client, project_id: i64) -> Result<String, String> {
    let row = db
        .query_opt(SQL_SELECT_PROJECT, &[&project_id])
        .await
        .map_err(|err| format!("select project failed: {err}"))?;
    let Some(row) = row else {
        return Err("project not found".to_string());
    };
    let name: String = row.get("name");
    let schema_row = db
        .query_one(SQL_PROJECT_SCHEMA_NAME, &[&project_id, &name])
        .await
        .map_err(|err| format!("project schema failed: {err}"))?;
    let schema: String = schema_row.get(0);
    Ok(schema)
}

pub async fn fetch_task_payload(
    db: &mut Client,
    schema: &str,
    task_id: i64,
) -> Result<TaskPayload, String> {
    let sql = task_payload_select_sql(schema);
    let row = db
        .query_opt(sql.as_str(), &[&task_id])
        .await
        .map_err(|err| format!("select task payload failed: {err}"))?;
    let Some(row) = row else {
        return Err("task not found".to_string());
    };
    let payload: serde_json::Value = row.get("payload");
    serde_json::from_value(payload).map_err(|err| format!("decode payload failed: {err}"))
}

pub async fn fetch_latest_result(
    db: &mut Client,
    schema: &str,
    task_id: i64,
) -> Result<serde_json::Value, String> {
    let sql = task_results_latest_sql(schema);
    let row = db
        .query_opt(sql.as_str(), &[&task_id])
        .await
        .map_err(|err| format!("select latest result failed: {err}"))?;
    let Some(row) = row else {
        return Err("result not found".to_string());
    };
    let result: serde_json::Value = row.get("result");
    Ok(result)
}

pub async fn shard_counts(
    db: &mut Client,
    schema: &str,
    group_id: &str,
) -> Result<(u64, u64), String> {
    let sql = task_group_count_sql(schema);
    let rows = db
        .query(sql.as_str(), &[&group_id])
        .await
        .map_err(|err| format!("group count failed: {err}"))?;

    let mut total = 0u64;
    let mut completed = 0u64;
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        let count = count.max(0) as u64;
        total += count;
        if status == "done" || status == "completed" {
            completed += count;
        }
    }
    Ok((total, completed))
}

pub async fn insert_audit_flag(
    db: &impl GenericClient,
    device_id: Option<i64>,
    task_id: i64,
    reason: &str,
    details: &serde_json::Value,
) -> Result<(), String> {
    db.execute(SQL_INSERT_FLAG, &[&device_id, &task_id, &reason, details])
        .await
        .map_err(|err| format!("insert audit failed: {err}"))?;
    Ok(())
}

pub async fn update_reputation(
    db: &impl GenericClient,
    device_id: i64,
    delta: f64,
) -> Result<f64, Error> {
    let row = db
        .query_one(SQL_UPSERT_REPUTATION, &[&device_id, &delta])
        .await?;
    let score: f64 = row.get(0);
    Ok(score)
}
