use tokio_postgres::{Client, GenericClient};

use crate::models::{Project, TaskQueueSummary, TaskSummary};

const SQL_LIST_PROJECTS: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects ORDER BY id";
const SQL_INSERT_PROJECT: &str = "INSERT INTO projects (name, description, owner_id) \
VALUES ($1, $2, $3) \
RETURNING id, name, description, owner_id, created_at::text AS created_at";
const SQL_SELECT_PROJECT: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects WHERE id = $1";
const SQL_DELETE_PROJECT: &str = "DELETE FROM projects WHERE id = $1";
const SQL_CREATE_PROJECT_SCHEMA: &str = "SELECT create_project_schema($1, $2)";
const SQL_DROP_PROJECT_SCHEMA: &str = "SELECT drop_project_schema($1, $2)";
const SQL_SELECT_PROJECT_BY_NAME: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects WHERE name = $1";
const SQL_PROJECT_SCHEMA_NAME: &str = "SELECT project_schema_name($1, $2)";

pub fn task_insert_sql(schema: &str) -> String {
    // Tasks live inside per-project schemas, so qualify table names.
    format!(
        "INSERT INTO {}.tasks (status, payload) VALUES ($1, $2) RETURNING id",
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
            name: row.get("name"),
            description: row.get("description"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
        })
        .collect())
}

pub async fn insert_project(
    db: &impl GenericClient,
    name: &str,
    description: &Option<String>,
    owner_id: &Option<i64>,
) -> Result<Project, tokio_postgres::Error> {
    let row = db
        .query_one(SQL_INSERT_PROJECT, &[&name, description, owner_id])
        .await?;

    Ok(Project {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
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
    db.execute(SQL_CREATE_PROJECT_SCHEMA, &[&project.id, &project.name])
        .await
        .map_err(|err| format!("create schema failed: {err}"))?;
    Ok(())
}

pub async fn drop_project_schema(db: &impl GenericClient, project: &Project) -> Result<(), String> {
    db.execute(SQL_DROP_PROJECT_SCHEMA, &[&project.id, &project.name])
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
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
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
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        created_at: row.get("created_at"),
    }))
}

pub async fn schema_name_for_project(db: &mut Client, project: &Project) -> Result<String, String> {
    let row = db
        .query_one(SQL_PROJECT_SCHEMA_NAME, &[&project.id, &project.name])
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
