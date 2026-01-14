mod policy;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use tokio_postgres::{error::SqlState, Client, NoTls};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::policy::{PolicyConfig, PolicyDecision, PolicyEngine, ProposalSource, TaskRequestProposal};

#[derive(Clone)]
struct AppState {
    policy: Arc<PolicyEngine>,
    db: Arc<Client>,
}

#[allow(dead_code)]
const SQL_INSERT_TASK: &str =
    "INSERT INTO tasks (project_id, status, payload) VALUES ($1, $2, $3) RETURNING id";
#[allow(dead_code)]
const SQL_UPDATE_TASK_STATUS: &str =
    "UPDATE tasks SET status = $1, updated_at = NOW() WHERE id = $2";
#[allow(dead_code)]
const SQL_INSERT_TASK_RESULT: &str =
    "INSERT INTO task_results (task_id, device_id, status, result) VALUES ($1, $2, $3, $4)";
#[allow(dead_code)]
const SQL_LIST_PROJECTS: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects ORDER BY id";
#[allow(dead_code)]
const SQL_INSERT_PROJECT: &str = "INSERT INTO projects (name, description, owner_id) \
VALUES ($1, $2, $3) \
RETURNING id, name, description, owner_id, created_at::text AS created_at";
#[allow(dead_code)]
const SQL_SELECT_PROJECT: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects WHERE id = $1";
#[allow(dead_code)]
const SQL_DELETE_PROJECT: &str = "DELETE FROM projects WHERE id = $1";
#[allow(dead_code)]
const SQL_CREATE_PROJECT_SCHEMA: &str = "SELECT create_project_schema($1, $2)";
#[allow(dead_code)]
const SQL_DROP_PROJECT_SCHEMA: &str = "SELECT drop_project_schema($1, $2)";
#[allow(dead_code)]
const SQL_SELECT_PROJECT_BY_NAME: &str =
    "SELECT id, name, description, owner_id, created_at::text AS created_at FROM projects WHERE name = $1";
#[allow(dead_code)]
const SQL_PROJECT_SCHEMA_NAME: &str = "SELECT project_schema_name($1, $2)";

#[allow(dead_code)]
fn task_insert_sql(schema: &str) -> String {
    // Tasks live inside per-project schemas, so qualify table names.
    format!(
        "INSERT INTO {}.tasks (status, payload) VALUES ($1, $2) RETURNING id",
        schema
    )
}

#[allow(dead_code)]
fn task_result_insert_sql(schema: &str) -> String {
    // Task results are scoped to the same project schema.
    format!(
        "INSERT INTO {}.task_results (task_id, device_id, status, result) VALUES ($1, $2, $3, $4)",
        schema
    )
}

fn task_select_next_sql(schema: &str) -> String {
    // Fetch next queued task using row-level locks for safe concurrency.
    format!(
        "SELECT id, payload FROM {}.tasks WHERE status = 'queued' ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED",
        schema
    )
}

fn task_update_status_sql(schema: &str) -> String {
    format!("UPDATE {}.tasks SET status = $1, updated_at = NOW() WHERE id = $2", schema)
}

fn task_clear_sql(schema: &str) -> String {
    format!("DELETE FROM {}.tasks", schema)
}

fn task_results_clear_sql(schema: &str) -> String {
    format!("DELETE FROM {}.task_results", schema)
}

fn task_status_counts_sql(schema: &str) -> String {
    format!(
        "SELECT status, COUNT(*) AS count FROM {}.tasks GROUP BY status",
        schema
    )
}

fn task_results_sql(schema: &str) -> String {
    format!("SELECT result FROM {}.task_results", schema)
}

fn task_results_allow_null_device_sql(schema: &str) -> String {
    format!(
        "ALTER TABLE {}.task_results ALTER COLUMN device_id DROP NOT NULL",
        schema
    )
}

#[derive(Deserialize)]
struct TaskRequest {
    node_id: Option<String>,
    requested_tasks: Option<u32>,
    proposal_source: Option<String>,
}

#[derive(Serialize)]
struct TaskResponse {
    status: &'static str,
    task_id: String,
    policy_decision: &'static str,
    granted_tasks: u32,
    reasons: Vec<String>,
    payload: Option<serde_json::Value>,
    project_id: Option<i64>,
}

#[derive(Deserialize)]
struct TaskSubmitRequest {
    task_id: String,
    result: Option<String>,
    project_id: Option<i64>,
    device_id: Option<i64>,
}

#[derive(Serialize)]
struct TaskSubmitResponse {
    status: &'static str,
}

#[derive(Deserialize)]
struct HeartbeatRequest {
    node_id: Option<String>,
}

#[derive(Serialize)]
struct HeartbeatResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: String,
    reasons: Vec<String>,
}

#[derive(Serialize)]
struct Project {
    id: i64,
    name: String,
    description: Option<String>,
    owner_id: Option<i64>,
    created_at: String,
}

#[derive(Deserialize)]
struct CreateProjectRequest {
    name: String,
    description: Option<String>,
    owner_id: Option<i64>,
}

#[derive(Serialize)]
struct CreateProjectResponse {
    status: &'static str,
    project: Project,
}

#[derive(Deserialize)]
struct DemoStartParams {
    parts: Option<usize>,
}

#[derive(Serialize)]
struct DemoStartResponse {
    status: &'static str,
    project_id: i64,
    total_tasks: usize,
}

#[derive(Serialize)]
struct DemoStatusResponse {
    total: usize,
    completed: usize,
    running: usize,
    queued: usize,
}

#[derive(Serialize)]
struct DemoResultResponse {
    total: usize,
    completed: usize,
    top_words: Vec<WordCount>,
}

#[derive(Serialize)]
struct WordCount {
    word: String,
    count: u64,
}

const DEMO_PROJECT_NAME: &str = "demo_wordcount";
const DEMO_SCRIPT: &str = r#"
import json

def main():
    with open("input.txt", "r", encoding="utf-8") as handle:
        text = handle.read().lower()
    words = []
    current = []
    for ch in text:
        if ch.isalnum():
            current.append(ch)
        else:
            if current:
                words.append("".join(current))
                current = []
    if current:
        words.append("".join(current))
    counts = {}
    for word in words:
        counts[word] = counts.get(word, 0) + 1
    print(json.dumps(counts))

if __name__ == "__main__":
    main()
"#;

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    // Database is required for project operations.
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");

    let (db, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect db");
    tokio::spawn(async move {
        // Drive the connection in the background.
        if let Err(err) = connection.await {
            tracing::error!(error = %err, "database connection error");
        }
    });

    // Load policy limits from env for deterministic enforcement.
    let policy_config = PolicyConfig::from_env();
    let policy = PolicyEngine::new(policy_config);
    let state = AppState {
        policy: Arc::new(policy),
        db: Arc::new(db),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/tasks/request", post(request_task))
        .route("/v1/tasks/submit", post(submit_task))
        .route("/v1/heartbeat", post(heartbeat))
        .route("/v1/projects", get(list_projects))
        .route("/v1/projects", post(create_project))
        .route("/v1/projects/:id", delete(delete_project))
        .route("/v1/demo/wordcount/start", post(start_demo_wordcount))
        .route("/v1/demo/wordcount/status", get(status_demo_wordcount))
        .route("/v1/demo/wordcount/result", get(result_demo_wordcount))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    // Bind on all interfaces for container compatibility.
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn readyz() -> StatusCode {
    StatusCode::OK
}

async fn request_task(
    State(state): State<AppState>,
    Json(payload): Json<TaskRequest>,
) -> impl IntoResponse {
    let requested_tasks = payload.requested_tasks.unwrap_or(1);
    let source = ProposalSource::from_optional(payload.proposal_source.as_deref());
    let proposal = TaskRequestProposal {
        requested_tasks,
        source,
    };

    // Evaluate against deterministic policy first.
    let decision = state.policy.evaluate_task_request(proposal);
    tracing::info!(
        node_id = payload.node_id.as_deref().unwrap_or("unknown"),
        requested_tasks = requested_tasks,
        policy_decision = decision.decision(),
        reasons = ?decision.reasons(),
        "policy evaluation"
    );

    match decision {
        PolicyDecision::Denied { reasons } => (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                code: "policy_denied",
                message: "request denied by policy".to_string(),
                reasons,
            }),
        )
            .into_response(),
        PolicyDecision::Limited {
            granted_tasks,
            reasons,
        } => match fetch_demo_task(&state).await {
            Ok(Some(task)) => (
                StatusCode::OK,
                Json(TaskResponse {
                    status: "ok",
                    task_id: task.id.to_string(),
                    policy_decision: "limit",
                    granted_tasks,
                    reasons,
                    payload: Some(task.payload),
                    project_id: Some(task.project_id),
                }),
            )
                .into_response(),
            Ok(None) => (
                StatusCode::OK,
                Json(TaskResponse {
                    status: "ok",
                    task_id: "".to_string(),
                    policy_decision: "limit",
                    granted_tasks: 0,
                    reasons,
                    payload: None,
                    project_id: None,
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response(),
        },
        PolicyDecision::Allowed { reasons } => match fetch_demo_task(&state).await {
            Ok(Some(task)) => (
                StatusCode::OK,
                Json(TaskResponse {
                    status: "ok",
                    task_id: task.id.to_string(),
                    policy_decision: "allow",
                    granted_tasks: 1,
                    reasons,
                    payload: Some(task.payload),
                    project_id: Some(task.project_id),
                }),
            )
                .into_response(),
            Ok(None) => (
                StatusCode::OK,
                Json(TaskResponse {
                    status: "ok",
                    task_id: "".to_string(),
                    policy_decision: "allow",
                    granted_tasks: 0,
                    reasons,
                    payload: None,
                    project_id: None,
                }),
            )
                .into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response(),
        },
    }
}

async fn submit_task(
    State(state): State<AppState>,
    Json(payload): Json<TaskSubmitRequest>,
) -> impl IntoResponse {
    // Persist task result into the project schema.
    let Some(project_id) = payload.project_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                code: "missing_project",
                message: "project_id is required".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    };

    let Some(result_raw) = payload.result else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                code: "missing_result",
                message: "result is required".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    };

    let project = match select_project_by_id(&state.db, project_id).await {
        Ok(Some(project)) => project,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    code: "project_not_found",
                    message: "project not found".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let schema = match schema_name_for_project(&state.db, &project).await {
        Ok(schema) => schema,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let result_value: serde_json::Value = serde_json::from_str(&result_raw)
        .unwrap_or_else(|_| serde_json::json!({ "raw": result_raw }));
    let device_id = payload.device_id;

    let transaction = match state.db.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "start transaction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let insert_sql = task_result_insert_sql(&schema);
    if let Err(err) = transaction
        .execute(
            insert_sql.as_str(),
            &[
                &payload.task_id.parse::<i64>().unwrap_or(0),
                &device_id,
                &"done",
                &result_value,
            ],
        )
        .await
    {
        tracing::error!(error = %err, "insert task result failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    let update_sql = task_update_status_sql(&schema);
    if let Err(err) = transaction
        .execute(update_sql.as_str(), &[&"done", &payload.task_id.parse::<i64>().unwrap_or(0)])
        .await
    {
        tracing::error!(error = %err, "update task status failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    Json(TaskSubmitResponse { status: "ok" }).into_response()
}

async fn heartbeat(Json(payload): Json<HeartbeatRequest>) -> Json<HeartbeatResponse> {
    // Heartbeats are logged for liveness tracking.
    tracing::info!(
        node_id = payload.node_id.as_deref().unwrap_or("unknown"),
        "heartbeat received"
    );
    Json(HeartbeatResponse { status: "ok" })
}

async fn start_demo_wordcount(
    State(state): State<AppState>,
    Query(params): Query<DemoStartParams>,
) -> impl IntoResponse {
    let parts = params.parts.unwrap_or(5).max(1);

    let project = match ensure_demo_project(&state.db).await {
        Ok(project) => project,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let schema = match schema_name_for_project(&state.db, &project).await {
        Ok(schema) => schema,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let text = generate_demo_text();
    let chunks = split_text(text.as_str(), parts);
    let total_tasks = chunks.len();

    let transaction = match state.db.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "start transaction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    if let Err(err) = transaction.execute(task_results_clear_sql(&schema).as_str(), &[]).await {
        tracing::error!(error = %err, "clear task_results failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    if let Err(err) = transaction.execute(task_clear_sql(&schema).as_str(), &[]).await {
        tracing::error!(error = %err, "clear tasks failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
            )
                .into_response();
    }

    if let Err(err) = transaction
        .execute(task_results_allow_null_device_sql(&schema).as_str(), &[])
        .await
    {
        tracing::warn!(error = %err, "ensure nullable device_id failed");
    }

    let insert_sql = task_insert_sql(&schema);
    for chunk in chunks {
        let payload = serde_json::json!({
            "kind": "python_script",
            "script": DEMO_SCRIPT,
            "inputs": {
                "input.txt": chunk
            }
        });
        if let Err(err) = transaction
            .query_one(insert_sql.as_str(), &[&"queued", &payload])
            .await
        {
            tracing::error!(error = %err, "insert task failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(DemoStartResponse {
            status: "ok",
            project_id: project.id,
            total_tasks,
        }),
    )
        .into_response()
}

async fn status_demo_wordcount(State(state): State<AppState>) -> impl IntoResponse {
    let project = match select_project_by_name(&state.db, DEMO_PROJECT_NAME).await {
        Ok(Some(project)) => project,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    code: "project_not_found",
                    message: "demo project not found".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let schema = match schema_name_for_project(&state.db, &project).await {
        Ok(schema) => schema,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let counts = match state.db.query(task_status_counts_sql(&schema).as_str(), &[]).await {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "status query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let mut total = 0;
    let mut completed = 0;
    let mut running = 0;
    let mut queued = 0;
    for row in counts {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        total += count;
        match status.as_str() {
            "done" => completed += count,
            "running" => running += count,
            "queued" => queued += count,
            _ => {}
        }
    }

    (
        StatusCode::OK,
        Json(DemoStatusResponse {
            total: total as usize,
            completed: completed as usize,
            running: running as usize,
            queued: queued as usize,
        }),
    )
        .into_response()
}

async fn result_demo_wordcount(State(state): State<AppState>) -> impl IntoResponse {
    let project = match select_project_by_name(&state.db, DEMO_PROJECT_NAME).await {
        Ok(Some(project)) => project,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    code: "project_not_found",
                    message: "demo project not found".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let schema = match schema_name_for_project(&state.db, &project).await {
        Ok(schema) => schema,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: err,
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let rows = match state.db.query(task_status_counts_sql(&schema).as_str(), &[]).await {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "status query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let mut total = 0;
    let mut completed = 0;
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        total += count;
        if status == "done" {
            completed += count;
        }
    }

    if total == 0 || completed < total {
        return (
            StatusCode::ACCEPTED,
            Json(DemoResultResponse {
                total: total as usize,
                completed: completed as usize,
                top_words: Vec::new(),
            }),
        )
            .into_response();
    }

    let result_rows = match state.db.query(task_results_sql(&schema).as_str(), &[]).await {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "results query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let mut aggregate: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for row in result_rows {
        let value: serde_json::Value = row.get("result");
        if let Some(map) = value.as_object() {
            for (word, count) in map {
                let count_val = count.as_u64().unwrap_or(0);
                *aggregate.entry(word.to_string()).or_insert(0) += count_val;
            }
        }
    }

    let mut top_words: Vec<WordCount> = aggregate
        .into_iter()
        .map(|(word, count)| WordCount { word, count })
        .collect();
    top_words.sort_by(|a, b| b.count.cmp(&a.count));
    top_words.truncate(10);

    (
        StatusCode::OK,
        Json(DemoResultResponse {
            total: total as usize,
            completed: completed as usize,
            top_words,
        }),
    )
        .into_response()
}

async fn fetch_demo_task(state: &AppState) -> Result<Option<DemoTask>, String> {
    let project = match select_project_by_name(&state.db, DEMO_PROJECT_NAME).await {
        Ok(Some(project)) => project,
        Ok(None) => return Ok(None),
        Err(err) => return Err(err),
    };

    let schema = schema_name_for_project(&state.db, &project).await?;

    let transaction = state
        .db
        .transaction()
        .await
        .map_err(|err| format!("start transaction failed: {err}"))?;

    let select_sql = task_select_next_sql(&schema);
    let row = transaction
        .query_opt(select_sql.as_str(), &[])
        .await
        .map_err(|err| format!("select task failed: {err}"))?;

    let Some(row) = row else {
        let _ = transaction.commit().await;
        return Ok(None);
    };

    let task_id: i64 = row.get("id");
    let payload: serde_json::Value = row.get("payload");

    let update_sql = task_update_status_sql(&schema);
    transaction
        .execute(update_sql.as_str(), &[&"running", &task_id])
        .await
        .map_err(|err| format!("update task failed: {err}"))?;

    transaction
        .commit()
        .await
        .map_err(|err| format!("commit failed: {err}"))?;

    Ok(Some(DemoTask {
        id: task_id,
        payload,
        project_id: project.id,
    }))
}

async fn ensure_demo_project(db: &Client) -> Result<Project, String> {
    if let Some(project) = select_project_by_name(db, DEMO_PROJECT_NAME).await? {
        return Ok(project);
    }

    let transaction = db
        .transaction()
        .await
        .map_err(|err| format!("start transaction failed: {err}"))?;

    let row = transaction
        .query_one(
            SQL_INSERT_PROJECT,
            &[&DEMO_PROJECT_NAME, &Some("Wordcount demo project".to_string()), &Option::<i64>::None],
        )
        .await
        .map_err(|err| format!("insert demo project failed: {err}"))?;

    let project = Project {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        created_at: row.get("created_at"),
    };

    transaction
        .execute(SQL_CREATE_PROJECT_SCHEMA, &[&project.id, &project.name])
        .await
        .map_err(|err| format!("create schema failed: {err}"))?;

    transaction
        .commit()
        .await
        .map_err(|err| format!("commit failed: {err}"))?;

    Ok(project)
}

async fn select_project_by_id(db: &Client, project_id: i64) -> Result<Option<Project>, String> {
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

async fn select_project_by_name(db: &Client, name: &str) -> Result<Option<Project>, String> {
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

async fn schema_name_for_project(db: &Client, project: &Project) -> Result<String, String> {
    let row = db
        .query_one(SQL_PROJECT_SCHEMA_NAME, &[&project.id, &project.name])
        .await
        .map_err(|err| format!("schema name failed: {err}"))?;
    let schema: String = row.get(0);
    if !is_safe_schema_name(&schema) {
        return Err("unsafe schema name".to_string());
    }
    Ok(schema)
}

fn is_safe_schema_name(name: &str) -> bool {
    // Ensure schema uses only safe characters.
    name.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.starts_with("prj_")
}

fn generate_demo_text() -> String {
    // Generate a deterministic text blob for the demo.
    let seed = "Newral orchestrates distributed compute with trust and verification.";
    let mut text = String::new();
    for _ in 0..400 {
        text.push_str(seed);
        text.push(' ');
    }
    text
}

fn split_text(text: &str, parts: usize) -> Vec<String> {
    // Split text into roughly equal word chunks.
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }
    let part_size = (words.len() + parts - 1) / parts;
    let mut chunks = Vec::new();
    for chunk in words.chunks(part_size) {
        chunks.push(chunk.join(" "));
    }
    chunks
}

struct DemoTask {
    id: i64,
    payload: serde_json::Value,
    project_id: i64,
}

async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    // Read project metadata from the public schema.
    let rows = match state.db.query(SQL_LIST_PROJECTS, &[]).await {
        Ok(rows) => rows,
        Err(err) => {
            tracing::error!(error = %err, "list projects failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let projects: Vec<Project> = rows
        .into_iter()
        .map(|row| Project {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
        })
        .collect();

    (StatusCode::OK, Json(projects)).into_response()
}

async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    // Create project metadata and its schema in a single transaction.
    let transaction = match state.db.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "start transaction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let row = match transaction
        .query_one(
            SQL_INSERT_PROJECT,
            &[&payload.name, &payload.description, &payload.owner_id],
        )
        .await
    {
        Ok(row) => row,
        Err(err) => {
            if let Some(db_err) = err.as_db_error() {
                if db_err.code() == &SqlState::UNIQUE_VIOLATION {
                    return (
                        StatusCode::CONFLICT,
                        Json(ErrorResponse {
                            code: "project_exists",
                            message: "project name already exists".to_string(),
                            reasons: Vec::new(),
                        }),
                    )
                        .into_response();
                }
            }
            tracing::error!(error = %err, "insert project failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let project = Project {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        owner_id: row.get("owner_id"),
        created_at: row.get("created_at"),
    };

    if let Err(err) = transaction
        .execute(SQL_CREATE_PROJECT_SCHEMA, &[&project.id, &project.name])
        .await
    {
        tracing::error!(error = %err, "create project schema failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(CreateProjectResponse {
            status: "ok",
            project,
        }),
    )
        .into_response()
}

async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> impl IntoResponse {
    // Delete schema and metadata in a single transaction.
    let transaction = match state.db.transaction().await {
        Ok(tx) => tx,
        Err(err) => {
            tracing::error!(error = %err, "start transaction failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let row = match transaction.query_opt(SQL_SELECT_PROJECT, &[&project_id]).await {
        Ok(row) => row,
        Err(err) => {
            tracing::error!(error = %err, "select project failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "db_error",
                    message: "database error".to_string(),
                    reasons: Vec::new(),
                }),
            )
                .into_response();
        }
    };

    let Some(row) = row else {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                code: "project_not_found",
                message: "project not found".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    };

    let project_name: String = row.get("name");

    if let Err(err) = transaction
        .execute(SQL_DROP_PROJECT_SCHEMA, &[&project_id, &project_name])
        .await
    {
        tracing::error!(error = %err, "drop project schema failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    if let Err(err) = transaction.execute(SQL_DELETE_PROJECT, &[&project_id]).await {
        tracing::error!(error = %err, "delete project failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                code: "db_error",
                message: "database error".to_string(),
                reasons: Vec::new(),
            }),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("sigterm handler");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        #[cfg(unix)]
        _ = sigterm.recv() => {},
    }
}
