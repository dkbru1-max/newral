use tokio_postgres::{Client, GenericClient};
use uuid::Uuid;

use crate::models::{Project, TaskQueueSummary, TaskSummary};

const SQL_LIST_PROJECTS: &str =
    "SELECT id, guid, name, description, owner_id, is_demo, storage_prefix, created_at::text AS created_at \
FROM projects ORDER BY id";
const SQL_INSERT_PROJECT: &str = "INSERT INTO projects (guid, name, description, owner_id, is_demo, storage_prefix) \
VALUES ($1, $2, $3, $4, $5, $6) \
RETURNING id, guid, name, description, owner_id, is_demo, storage_prefix, created_at::text AS created_at";
const SQL_SELECT_PROJECT: &str =
    "SELECT id, guid, name, description, owner_id, is_demo, storage_prefix, created_at::text AS created_at \
FROM projects WHERE id = $1";
const SQL_DELETE_PROJECT: &str = "DELETE FROM projects WHERE id = $1";
const SQL_CREATE_PROJECT_SCHEMA: &str = "SELECT create_project_schema($1, $2, $3)";
const SQL_DROP_PROJECT_SCHEMA: &str = "SELECT drop_project_schema($1, $2, $3)";
const SQL_SELECT_PROJECT_BY_NAME: &str =
    "SELECT id, guid, name, description, owner_id, is_demo, storage_prefix, created_at::text AS created_at \
FROM projects WHERE name = $1";
const SQL_PROJECT_SCHEMA_NAME: &str = "SELECT project_schema_name($1, $2, $3)";
const SQL_UPSERT_AGENT: &str = "INSERT INTO agents (agent_uid, display_name, cpu_limit_percent, gpu_limit_percent, ram_limit_percent, last_seen) \
VALUES ($1, $2, $3, $4, $5, NOW()) \
ON CONFLICT (agent_uid) DO UPDATE SET \
display_name = COALESCE(EXCLUDED.display_name, agents.display_name), \
cpu_limit_percent = COALESCE(EXCLUDED.cpu_limit_percent, agents.cpu_limit_percent), \
gpu_limit_percent = COALESCE(EXCLUDED.gpu_limit_percent, agents.gpu_limit_percent), \
ram_limit_percent = COALESCE(EXCLUDED.ram_limit_percent, agents.ram_limit_percent), \
last_seen = NOW(), \
updated_at = NOW() \
RETURNING id, blocked, blocked_reason";
const SQL_SELECT_AGENT: &str = "SELECT id, blocked, blocked_reason FROM agents WHERE agent_uid = $1";
const SQL_UPSERT_HARDWARE: &str = "INSERT INTO agent_hardware (agent_id, hardware) \
VALUES ($1, $2) \
ON CONFLICT (agent_id) DO UPDATE SET hardware = EXCLUDED.hardware, updated_at = NOW()";
const SQL_INSERT_METRICS: &str = "INSERT INTO agent_metrics \
(agent_id, cpu_load, ram_used_mb, ram_total_mb, gpu_load, gpu_mem_used_mb, net_rx_bytes, net_tx_bytes, disk_read_bytes, disk_write_bytes) \
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)";
const SQL_UPSERT_TASK_TYPE: &str = "INSERT INTO project_task_types \
(project_id, task_type, script_object_key, script_sha256, script_version) \
VALUES ($1, $2, $3, $4, $5) \
ON CONFLICT (project_id, task_type) DO UPDATE SET \
script_object_key = EXCLUDED.script_object_key, \
script_sha256 = EXCLUDED.script_sha256, \
script_version = EXCLUDED.script_version, \
updated_at = NOW()";

pub fn task_insert_sql(schema: &str) -> String {
    // Tasks live inside per-project schemas, so qualify table names.
    format!(
        "INSERT INTO {}.tasks (status, task_type, payload) VALUES ($1, $2, $3) RETURNING id",
        schema
    )
}

pub fn task_result_insert_sql(schema: &str) -> String {
    // Task results are scoped to the same project schema.
    format!(
        "INSERT INTO {}.task_results (task_id, device_id, status, result) VALUES ($1, $2, $3, $4)",
        schema
    )
}

pub fn task_select_next_sql(schema: &str) -> String {
    // Fetch next queued task using row-level locks for safe concurrency.
    format!(
        "SELECT id, payload FROM {}.tasks WHERE status = 'queued' ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED",
        schema
    )
}

pub fn task_update_status_sql(schema: &str) -> String {
    format!(
        "UPDATE {}.tasks SET status = $1, updated_at = NOW() WHERE id = $2",
        schema
    )
}

pub fn task_clear_sql(schema: &str) -> String {
    format!("DELETE FROM {}.tasks", schema)
}

pub fn task_results_clear_sql(schema: &str) -> String {
    format!("DELETE FROM {}.task_results", schema)
}

pub fn task_status_counts_sql(schema: &str) -> String {
    format!(
        "SELECT status, COUNT(*) AS count FROM {}.tasks GROUP BY status",
        schema
    )
}

pub fn task_results_sql(schema: &str) -> String {
    format!("SELECT result FROM {}.task_results", schema)
}

pub fn task_results_allow_null_device_sql(schema: &str) -> String {
    format!(
        "ALTER TABLE {}.task_results ALTER COLUMN device_id DROP NOT NULL",
        schema
    )
}

pub fn task_followup_exists_sql(schema: &str) -> String {
    format!(
        "SELECT 1 FROM {}.tasks WHERE payload->>'kind' = 'followup_report' LIMIT 1",
        schema
    )
}

pub async fn list_projects(db: &mut Client) -> Result<Vec<Project>, String> {
    let rows = db
        .query(SQL_LIST_PROJECTS, &[])
        .await
        .map_err(|err| format!("list projects failed: {err}"))?;

    Ok(rows
        .into_iter()
        .map(|row| Project {
            id: row.get("id"),
            guid: row.get("guid"),
            name: row.get("name"),
            description: row.get("description"),
            owner_id: row.get("owner_id"),
            is_demo: row.get("is_demo"),
            storage_prefix: row.get("storage_prefix"),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub async fn insert_project(
    db: &impl GenericClient,
    guid: &Uuid,
    name: &str,
    description: &Option<String>,
    owner_id: &Option<i64>,
    is_demo: bool,
    storage_prefix: &str,
) -> Result<Project, tokio_postgres::Error> {
    let row = db
        .query_one(
            SQL_INSERT_PROJECT,
            &[guid, &name, description, owner_id, &is_demo, &storage_prefix],
        )
        .await?;

    Ok(Project {
        id: row.get("id"),
        guid: row.get("guid"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        is_demo: row.get("is_demo"),
        storage_prefix: row.get("storage_prefix"),
        created_at: row.get("created_at"),
    })
}

pub async fn delete_project(db: &impl GenericClient, project_id: i64) -> Result<(), String> {
    db.execute(SQL_DELETE_PROJECT, &[&project_id])
        .await
        .map_err(|err| format!("delete project failed: {err}"))?;
    Ok(())
}

pub async fn create_project_schema(
    db: &impl GenericClient,
    project: &Project,
) -> Result<(), String> {
    db.execute(
        SQL_CREATE_PROJECT_SCHEMA,
        &[&project.id, &project.name, &project.guid],
    )
    .await
    .map_err(|err| format!("create schema failed: {err}"))?;
    Ok(())
}

pub async fn drop_project_schema(db: &impl GenericClient, project: &Project) -> Result<(), String> {
    db.execute(
        SQL_DROP_PROJECT_SCHEMA,
        &[&project.id, &project.name, &project.guid],
    )
    .await
    .map_err(|err| format!("drop schema failed: {err}"))?;
    Ok(())
}

pub async fn select_project_by_id(
    db: &mut Client,
    project_id: i64,
) -> Result<Option<Project>, String> {
    let row = db
        .query_opt(SQL_SELECT_PROJECT, &[&project_id])
        .await
        .map_err(|err| format!("select project failed: {err}"))?;

    Ok(row.map(|row| Project {
        id: row.get("id"),
        guid: row.get("guid"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        is_demo: row.get("is_demo"),
        storage_prefix: row.get("storage_prefix"),
        created_at: row.get("created_at"),
    }))
}

pub async fn select_project_by_name(
    db: &mut Client,
    name: &str,
) -> Result<Option<Project>, String> {
    let row = db
        .query_opt(SQL_SELECT_PROJECT_BY_NAME, &[&name])
        .await
        .map_err(|err| format!("select project by name failed: {err}"))?;

    Ok(row.map(|row| Project {
        id: row.get("id"),
        guid: row.get("guid"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        is_demo: row.get("is_demo"),
        storage_prefix: row.get("storage_prefix"),
        created_at: row.get("created_at"),
    }))
}

pub async fn schema_name_for_project(db: &mut Client, project: &Project) -> Result<String, String> {
    let row = db
        .query_one(SQL_PROJECT_SCHEMA_NAME, &[&project.id, &project.name, &project.guid])
        .await
        .map_err(|err| format!("project schema failed: {err}"))?;
    let schema: String = row.get(0);
    Ok(schema)
}

pub async fn fetch_recent_tasks(db: &mut Client, schema: &str) -> Result<Vec<TaskSummary>, String> {
    let sql = format!(
        "SELECT id, status FROM {}.tasks ORDER BY updated_at DESC, id DESC LIMIT 6",
        schema
    );
    let rows = db
        .query(sql.as_str(), &[])
        .await
        .map_err(|err| format!("list tasks failed: {err}"))?;

    Ok(rows
        .into_iter()
        .map(|row| TaskSummary {
            id: format!("task-{}", row.get::<_, i64>("id")),
            status: row.get::<_, String>("status"),
            priority: "normal".to_string(),
        })
        .collect())
}

pub async fn task_counts(db: &mut Client, schema: &str) -> Result<TaskQueueSummary, String> {
    let sql = task_status_counts_sql(schema);
    let rows = db
        .query(sql.as_str(), &[])
        .await
        .map_err(|err| format!("status counts failed: {err}"))?;

    let mut summary = TaskQueueSummary::default();
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        match status.as_str() {
            "queued" => summary.queued += count.max(0) as u64,
            "running" => summary.running += count.max(0) as u64,
            "done" | "completed" => summary.completed += count.max(0) as u64,
            _ => {}
        }
    }
    Ok(summary)
}

pub async fn task_completed_last_min(db: &mut Client, schema: &str) -> Result<u64, String> {
    let sql = format!(
        "SELECT COUNT(*) AS count FROM {}.tasks WHERE status IN ('done', 'completed') AND updated_at > NOW() - interval '1 minute'",
        schema
    );
    let row = db
        .query_one(sql.as_str(), &[])
        .await
        .map_err(|err| format!("last minute count failed: {err}"))?;
    let count: i64 = row.get("count");
    Ok(count.max(0) as u64)
}

pub struct AgentRecord {
    pub id: i64,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

pub async fn upsert_agent(
    db: &impl GenericClient,
    agent_uid: &Uuid,
    display_name: &Option<String>,
    cpu_limit_percent: Option<f32>,
    gpu_limit_percent: Option<f32>,
    ram_limit_percent: Option<f32>,
) -> Result<AgentRecord, String> {
    let row = db
        .query_one(
            SQL_UPSERT_AGENT,
            &[
                agent_uid,
                display_name,
                &cpu_limit_percent,
                &gpu_limit_percent,
                &ram_limit_percent,
            ],
        )
        .await
        .map_err(|err| format!("upsert agent failed: {err}"))?;
    Ok(AgentRecord {
        id: row.get("id"),
        blocked: row.get("blocked"),
        blocked_reason: row.get("blocked_reason"),
    })
}

pub async fn select_agent(
    db: &impl GenericClient,
    agent_uid: &Uuid,
) -> Result<Option<AgentRecord>, String> {
    let row = db
        .query_opt(SQL_SELECT_AGENT, &[agent_uid])
        .await
        .map_err(|err| format!("select agent failed: {err}"))?;
    Ok(row.map(|row| AgentRecord {
        id: row.get("id"),
        blocked: row.get("blocked"),
        blocked_reason: row.get("blocked_reason"),
    }))
}

pub async fn upsert_agent_hardware(
    db: &impl GenericClient,
    agent_id: i64,
    hardware: &serde_json::Value,
) -> Result<(), String> {
    db.execute(SQL_UPSERT_HARDWARE, &[&agent_id, hardware])
        .await
        .map_err(|err| format!("upsert hardware failed: {err}"))?;
    Ok(())
}

pub async fn insert_agent_metrics(
    db: &impl GenericClient,
    agent_id: i64,
    metrics: &crate::models::AgentMetrics,
) -> Result<(), String> {
    db.execute(
        SQL_INSERT_METRICS,
        &[
            &agent_id,
            &metrics.cpu_load,
            &metrics.ram_used_mb,
            &metrics.ram_total_mb,
            &metrics.gpu_load,
            &metrics.gpu_mem_used_mb,
            &metrics.net_rx_bytes,
            &metrics.net_tx_bytes,
            &metrics.disk_read_bytes,
            &metrics.disk_write_bytes,
        ],
    )
    .await
    .map_err(|err| format!("insert metrics failed: {err}"))?;
    Ok(())
}

pub async fn upsert_agent_preferences(
    db: &impl GenericClient,
    agent_id: i64,
    project_id: i64,
    allowed_task_types: &[String],
) -> Result<(), String> {
    db.execute(
        "INSERT INTO agent_project_preferences (agent_id, project_id, allowed_task_types) \
        VALUES ($1, $2, $3) \
        ON CONFLICT (agent_id, project_id) DO UPDATE SET allowed_task_types = EXCLUDED.allowed_task_types, updated_at = NOW()",
        &[&agent_id, &project_id, &allowed_task_types],
    )
    .await
    .map_err(|err| format!("upsert preferences failed: {err}"))?;
    Ok(())
}

pub async fn fetch_agent_preferences(
    db: &mut Client,
    agent_id: i64,
    project_id: i64,
) -> Result<Option<Vec<String>>, String> {
    let row = db
        .query_opt(
            "SELECT allowed_task_types FROM agent_project_preferences WHERE agent_id = $1 AND project_id = $2",
            &[&agent_id, &project_id],
        )
        .await
        .map_err(|err| format!("fetch preferences failed: {err}"))?;
    Ok(row.map(|row| row.get("allowed_task_types")))
}

pub async fn upsert_project_task_type(
    db: &impl GenericClient,
    project_id: i64,
    task_type: &str,
    script_object_key: &str,
    script_sha256: &str,
    script_version: Option<&str>,
) -> Result<(), String> {
    db.execute(
        SQL_UPSERT_TASK_TYPE,
        &[
            &project_id,
            &task_type,
            &script_object_key,
            &script_sha256,
            &script_version,
        ],
    )
    .await
    .map_err(|err| format!("upsert task type failed: {err}"))?;
    Ok(())
}

pub async fn fetch_task_types(
    db: &mut Client,
    project_id: i64,
) -> Result<Vec<(String, String, String)>, String> {
    let rows = db
        .query(
            "SELECT task_type, script_object_key, script_sha256 FROM project_task_types WHERE project_id = $1",
            &[&project_id],
        )
        .await
        .map_err(|err| format!("fetch task types failed: {err}"))?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}
