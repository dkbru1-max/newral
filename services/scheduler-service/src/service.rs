use axum::http::StatusCode;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_postgres::error::SqlState;

use crate::db;
use crate::models::{
    AgentSummary, CreateProjectRequest, CreateProjectResponse, DemoResultResponse, DemoStartParams,
    DemoStartResponse, DemoStatusResponse, ErrorResponse, HeartbeatRequest, HeartbeatResponse,
    LiveSummary, Project, TaskQueueSummary, TaskRequest, TaskResponse, TaskSubmitRequest,
    TaskSubmitResponse, WordCount,
};
use crate::policy::{PolicyDecision, ProposalSource, TaskRequestProposal};
use crate::state::{AgentHeartbeat, AppState};

const DEMO_PROJECT_NAME: &str = "demo_wordcount";
const DEMO_SCRIPT: &str = r#"
import json

with open("input.txt", "r", encoding="utf-8") as handle:
    words = handle.read().split()

result = {}
for word in words:
    word = word.lower()
    result[word] = result.get(word, 0) + 1

print(json.dumps(result))
"#;

pub struct ServiceError {
    pub status: StatusCode,
    pub body: ErrorResponse,
}

impl ServiceError {
    pub fn new(status: StatusCode, code: &'static str, message: String) -> Self {
        Self {
            status,
            body: ErrorResponse {
                code,
                message,
                reasons: Vec::new(),
            },
        }
    }

    pub fn with_reasons(
        status: StatusCode,
        code: &'static str,
        message: String,
        reasons: Vec<String>,
    ) -> Self {
        Self {
            status,
            body: ErrorResponse {
                code,
                message,
                reasons,
            },
        }
    }
}

pub fn notify_update(state: &AppState) {
    let _ = state.updates.send(());
}

pub async fn heartbeat(
    state: &AppState,
    payload: HeartbeatRequest,
) -> Result<HeartbeatResponse, ServiceError> {
    let node_id = payload.node_id.as_deref().unwrap_or("unknown").to_string();
    {
        let mut heartbeats = state.heartbeats.lock().await;
        heartbeats.insert(
            node_id.clone(),
            AgentHeartbeat {
                last_seen: SystemTime::now(),
            },
        );
    }

    tracing::info!(node_id = node_id.as_str(), "heartbeat received");
    notify_update(state);
    Ok(HeartbeatResponse { status: "ok" })
}

pub async fn request_task(
    state: &AppState,
    payload: TaskRequest,
) -> Result<TaskResponse, ServiceError> {
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

    let response = match decision {
        PolicyDecision::Denied { reasons } => {
            return Err(ServiceError::with_reasons(
                StatusCode::FORBIDDEN,
                "policy_denied",
                "request denied by policy".to_string(),
                reasons,
            ));
        }
        PolicyDecision::Limited {
            granted_tasks,
            reasons,
        } => {
            let mut db = state.db.lock().await;
            match fetch_demo_task(&mut db).await {
                Ok(Some(task)) => TaskResponse {
                    status: "ok",
                    task_id: task.id.to_string(),
                    policy_decision: "limit",
                    granted_tasks,
                    reasons,
                    payload: Some(task.payload),
                    project_id: Some(task.project_id),
                },
                Ok(None) => TaskResponse {
                    status: "ok",
                    task_id: "".to_string(),
                    policy_decision: "limit",
                    granted_tasks: 0,
                    reasons,
                    payload: None,
                    project_id: None,
                },
                Err(err) => {
                    return Err(ServiceError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "db_error",
                        err,
                    ));
                }
            }
        }
        PolicyDecision::Allowed { reasons } => {
            let mut db = state.db.lock().await;
            match fetch_demo_task(&mut db).await {
                Ok(Some(task)) => TaskResponse {
                    status: "ok",
                    task_id: task.id.to_string(),
                    policy_decision: "allow",
                    granted_tasks: 1,
                    reasons,
                    payload: Some(task.payload),
                    project_id: Some(task.project_id),
                },
                Ok(None) => TaskResponse {
                    status: "ok",
                    task_id: "".to_string(),
                    policy_decision: "allow",
                    granted_tasks: 0,
                    reasons,
                    payload: None,
                    project_id: None,
                },
                Err(err) => {
                    return Err(ServiceError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "db_error",
                        err,
                    ));
                }
            }
        }
    };

    notify_update(state);
    Ok(response)
}

pub async fn submit_task(
    state: &AppState,
    payload: TaskSubmitRequest,
) -> Result<TaskSubmitResponse, ServiceError> {
    let Some(project_id) = payload.project_id else {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "missing_project",
            "project_id is required".to_string(),
        ));
    };

    let Some(result_raw) = payload.result else {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "missing_result",
            "result is required".to_string(),
        ));
    };

    let mut db = state.db.lock().await;
    let project = match db::select_project_by_id(&mut db, project_id).await {
        Ok(Some(project)) => project,
        Ok(None) => {
            return Err(ServiceError::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                "project not found".to_string(),
            ));
        }
        Err(err) => {
            return Err(ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                err,
            ));
        }
    };

    let schema = schema_name_for_project(&mut db, &project).await?;

    let result_value: serde_json::Value = serde_json::from_str(&result_raw)
        .unwrap_or_else(|_| serde_json::json!({ "raw": result_raw }));
    let device_id = payload.device_id;

    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    let insert_sql = db::task_result_insert_sql(&schema);
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
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    let update_sql = db::task_update_status_sql(&schema);
    if let Err(err) = transaction
        .execute(
            update_sql.as_str(),
            &[&"done", &payload.task_id.parse::<i64>().unwrap_or(0)],
        )
        .await
    {
        tracing::error!(error = %err, "update task status failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    notify_update(state);
    Ok(TaskSubmitResponse { status: "ok" })
}

pub async fn list_projects(state: &AppState) -> Result<Vec<Project>, ServiceError> {
    let mut db = state.db.lock().await;
    db::list_projects(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))
}

pub async fn create_project(
    state: &AppState,
    payload: CreateProjectRequest,
) -> Result<CreateProjectResponse, ServiceError> {
    let mut db = state.db.lock().await;
    let CreateProjectRequest {
        name,
        description,
        owner_id,
    } = payload;
    let Some(name) = name else {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "missing_name",
            "name is required".to_string(),
        ));
    };

    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    let project = match db::insert_project(&transaction, &name, &description, &owner_id).await {
        Ok(project) => project,
        Err(err) => {
            if let Some(db_err) = err.as_db_error() {
                if db_err.code() == &SqlState::UNIQUE_VIOLATION {
                    return Err(ServiceError::new(
                        StatusCode::CONFLICT,
                        "project_exists",
                        "project name already exists".to_string(),
                    ));
                }
            }
            tracing::error!(error = %err, "insert project failed");
            return Err(ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                "database error".to_string(),
            ));
        }
    };

    if let Err(err) = db::create_project_schema(&transaction, &project).await {
        tracing::error!(error = %err, "create project schema failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    Ok(CreateProjectResponse {
        status: "ok",
        project: crate::models::ProjectResponse {
            id: project.id,
            name: project.name,
            description: project.description,
            owner_id: project.owner_id,
            created_at: project.created_at,
        },
    })
}

pub async fn delete_project(state: &AppState, project_id: i64) -> Result<(), ServiceError> {
    let mut db = state.db.lock().await;
    let project = db::select_project_by_id(&mut db, project_id)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
        .ok_or_else(|| {
            ServiceError::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                "project not found".to_string(),
            )
        })?;

    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    if let Err(err) = db::drop_project_schema(&transaction, &project).await {
        tracing::error!(error = %err, "drop project schema failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = db::delete_project(&transaction, project_id).await {
        tracing::error!(error = %err, "delete project failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    Ok(())
}

pub async fn start_demo_wordcount(
    state: &AppState,
    params: DemoStartParams,
) -> Result<DemoStartResponse, ServiceError> {
    let parts = params.parts.unwrap_or(5).max(1);

    let mut db = state.db.lock().await;
    let project = ensure_demo_project(&mut db).await?;
    let schema = schema_name_for_project(&mut db, &project).await?;

    let text = generate_demo_text();
    let chunks = split_text(text.as_str(), parts);
    let total_tasks = chunks.len();
    let group_id = format!(
        "demo-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    if let Err(err) = transaction
        .execute(db::task_results_clear_sql(&schema).as_str(), &[])
        .await
    {
        tracing::error!(error = %err, "clear task_results failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = transaction
        .execute(db::task_clear_sql(&schema).as_str(), &[])
        .await
    {
        tracing::error!(error = %err, "clear tasks failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = transaction
        .execute(
            db::task_results_allow_null_device_sql(&schema).as_str(),
            &[],
        )
        .await
    {
        tracing::warn!(error = %err, "ensure nullable device_id failed");
    }

    let insert_sql = db::task_insert_sql(&schema);
    let parent_payload = json!({
        "kind": "group",
        "group_id": group_id.clone(),
        "total": total_tasks,
    });
    let parent_row = transaction
        .query_one(insert_sql.as_str(), &[&"group", &parent_payload])
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "insert parent task failed");
            ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                "database error".to_string(),
            )
        })?;

    let parent_task_id: i64 = parent_row.get("id");
    for chunk in chunks {
        let payload = json!({
            "kind": "python_script",
            "script": DEMO_SCRIPT,
            "group_id": group_id.clone(),
            "parent_task_id": parent_task_id,
            "inputs": {
                "input.txt": chunk
            }
        });
        transaction
            .query_one(insert_sql.as_str(), &[&"queued", &payload])
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "insert task failed");
                ServiceError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "db_error",
                    "database error".to_string(),
                )
            })?;
    }

    transaction.commit().await.map_err(|err| {
        tracing::error!(error = %err, "commit failed");
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    notify_update(state);
    Ok(DemoStartResponse {
        status: "ok",
        project_id: project.id,
        total_tasks,
        group_id,
        parent_task_id,
    })
}

pub async fn status_demo_wordcount(state: &AppState) -> Result<DemoStatusResponse, ServiceError> {
    let mut db = state.db.lock().await;
    let project = db::select_project_by_name(&mut db, DEMO_PROJECT_NAME)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
        .ok_or_else(|| {
            ServiceError::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                "demo project not found".to_string(),
            )
        })?;

    let schema = schema_name_for_project(&mut db, &project).await?;
    let counts = db
        .query(db::task_status_counts_sql(&schema).as_str(), &[])
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "status query failed");
            ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                "database error".to_string(),
            )
        })?;

    let mut total = 0;
    let mut completed = 0;
    let mut running = 0;
    let mut queued = 0;
    for row in counts {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        match status.as_str() {
            "done" => {
                completed += count;
                total += count;
            }
            "running" => {
                running += count;
                total += count;
            }
            "queued" => {
                queued += count;
                total += count;
            }
            _ => {}
        }
    }

    Ok(DemoStatusResponse {
        total: total as usize,
        completed: completed as usize,
        running: running as usize,
        queued: queued as usize,
    })
}

pub async fn result_demo_wordcount(
    state: &AppState,
) -> Result<(StatusCode, DemoResultResponse), ServiceError> {
    let mut db = state.db.lock().await;
    let project = db::select_project_by_name(&mut db, DEMO_PROJECT_NAME)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
        .ok_or_else(|| {
            ServiceError::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                "demo project not found".to_string(),
            )
        })?;

    let schema = schema_name_for_project(&mut db, &project).await?;
    let rows = db
        .query(db::task_status_counts_sql(&schema).as_str(), &[])
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "status query failed");
            ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                "database error".to_string(),
            )
        })?;

    let mut total = 0;
    let mut completed = 0;
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        if status == "done" {
            completed += count;
            total += count;
        } else if status == "running" || status == "queued" {
            total += count;
        }
    }

    if total == 0 || completed < total {
        return Ok((
            StatusCode::ACCEPTED,
            DemoResultResponse {
                total: total as usize,
                completed: completed as usize,
                top_words: Vec::new(),
            },
        ));
    }

    let result_rows = db
        .query(db::task_results_sql(&schema).as_str(), &[])
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "results query failed");
            ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "db_error",
                "database error".to_string(),
            )
        })?;

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

    if total > 0 && completed >= total {
        if let Err(err) = plan_followup_task(&mut db, &schema, &top_words).await {
            tracing::warn!(error = %err, "followup task planning failed");
        }
    }

    Ok((
        StatusCode::OK,
        DemoResultResponse {
            total: total as usize,
            completed: completed as usize,
            top_words,
        },
    ))
}

pub async fn build_live_summary(state: &AppState) -> Result<LiveSummary, ServiceError> {
    let now = SystemTime::now();
    let agents = {
        let heartbeats = state.heartbeats.lock().await;
        heartbeats
            .iter()
            .map(|(id, heartbeat)| {
                let elapsed = now
                    .duration_since(heartbeat.last_seen)
                    .unwrap_or_default()
                    .as_secs();
                let status = if elapsed <= state.heartbeat_ttl.as_secs() {
                    "online"
                } else {
                    "idle"
                };
                AgentSummary {
                    id: id.clone(),
                    status: status.to_string(),
                    last_seen_secs: elapsed,
                    region: "local".to_string(),
                    reputation: "0.8".to_string(),
                }
            })
            .collect::<Vec<_>>()
    };

    let mut db = state.db.lock().await;
    let projects = db::list_projects(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    let mut queue = TaskQueueSummary::default();
    let mut load = crate::models::LoadSummary::default();
    let mut tasks = Vec::new();

    for project in projects.iter() {
        let schema = schema_name_for_project(&mut db, project).await?;
        let counts = db::task_counts(&mut db, &schema)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
        queue.queued += counts.queued;
        queue.running += counts.running;
        queue.completed += counts.completed;

        load.running += counts.running;
        load.queued += counts.queued;
        load.completed_last_min += db::task_completed_last_min(&mut db, &schema)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

        if tasks.is_empty() {
            tasks = db::fetch_recent_tasks(&mut db, &schema)
                .await
                .map_err(|err| {
                    ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err)
                })?;
        }
    }

    let ai_mode = state.policy.config().ai_mode.to_string();
    let updated_at = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    Ok(LiveSummary {
        updated_at,
        ai_mode,
        agents,
        tasks,
        queue,
        load,
    })
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
    let part_size = words.len().div_ceil(parts);
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

async fn fetch_demo_task(db: &mut tokio_postgres::Client) -> Result<Option<DemoTask>, String> {
    let project = match db::select_project_by_name(db, DEMO_PROJECT_NAME).await? {
        Some(project) => project,
        None => return Ok(None),
    };

    let schema = schema_name_for_project(db, &project)
        .await
        .map_err(|err| err.body.message)?;

    let transaction = db
        .transaction()
        .await
        .map_err(|err| format!("start transaction failed: {err}"))?;

    let select_sql = db::task_select_next_sql(&schema);
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

    let update_sql = db::task_update_status_sql(&schema);
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

async fn ensure_demo_project(db: &mut tokio_postgres::Client) -> Result<Project, ServiceError> {
    if let Some(project) = db::select_project_by_name(db, DEMO_PROJECT_NAME)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
    {
        return Ok(project);
    }

    let transaction = db.transaction().await.map_err(|err| {
        tracing::error!(error = %err, "start transaction failed");
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    let project = db::insert_project(
        &transaction,
        DEMO_PROJECT_NAME,
        &Some("Wordcount demo project".to_string()),
        &Option::<i64>::None,
    )
    .await
    .map_err(|err| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            format!("{err}"),
        )
    })?;

    if let Err(err) = db::create_project_schema(&transaction, &project).await {
        tracing::error!(error = %err, "create schema failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    transaction.commit().await.map_err(|err| {
        tracing::error!(error = %err, "commit failed");
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    Ok(project)
}

async fn schema_name_for_project(
    db: &mut tokio_postgres::Client,
    project: &Project,
) -> Result<String, ServiceError> {
    let schema = db::schema_name_for_project(db, project)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    if !is_safe_schema_name(&schema) {
        return Err(ServiceError::new(
            StatusCode::BAD_REQUEST,
            "unsafe_schema",
            "unsafe schema name".to_string(),
        ));
    }
    Ok(schema)
}

async fn plan_followup_task(
    db: &mut tokio_postgres::Client,
    schema: &str,
    top_words: &[WordCount],
) -> Result<(), String> {
    if std::env::var("DEMO_FOLLOWUP_ENABLED")
        .ok()
        .map(|value| value == "0")
        .unwrap_or(false)
    {
        return Ok(());
    }

    let exists = db
        .query_opt(db::task_followup_exists_sql(schema).as_str(), &[])
        .await
        .map_err(|err| format!("followup exists failed: {err}"))?;
    if exists.is_some() {
        return Ok(());
    }

    let report = json!({
        "top_words": top_words,
    });
    let script = r#"import json

with open("summary.json", "r", encoding="utf-8") as handle:
    payload = json.load(handle)

print("Top words")
for entry in payload.get("top_words", []):
    print(f"- {entry['word']}: {entry['count']}")
"#;

    let payload = json!({
        "kind": "followup_report",
        "script": script,
        "inputs": {
            "summary.json": report.to_string()
        }
    });

    db.execute(db::task_insert_sql(schema).as_str(), &[&"queued", &payload])
        .await
        .map_err(|err| format!("insert followup failed: {err}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::split_text;

    #[test]
    fn splits_text_into_chunks() {
        let text = "one two three four five";
        let chunks = split_text(text, 2);
        assert_eq!(chunks.len(), 2);
    }
}
