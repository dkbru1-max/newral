use axum::http::StatusCode;
use sha2::{Digest, Sha256};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_postgres::error::SqlState;

use crate::db;
use crate::models::{
    AgentSummary, BpswBound, CreateProjectRequest, CreateProjectResponse, DashboardPoint,
    DashboardSnapshot, DemoResultResponse, DemoStartParams, DemoStartResponse, DemoStatusResponse,
    ErrorResponse, HeartbeatRequest, HeartbeatResponse, LiveSummary, Project, TaskAssignment,
    TaskBatchRequest, TaskBatchResponse, TaskQueueSummary, TaskRequest, TaskResponse,
    TaskSubmitRequest, TaskSubmitResponse, ThroughputSnapshot, WordCount,
};
use crate::policy::{PolicyDecision, ProposalSource, TaskRequestProposal};
use newral_common::env_or;
use crate::state::{AgentHeartbeat, AppState};
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};
use uuid::Uuid;

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
const BPSW_PROJECT_NAME: &str = "bpsw_hunter";
const BPSW_SCRIPT_FILENAME: &str = "bpsw_worker.py";
const BPSW_TASK_TYPES: [&str; 6] = [
    "main_odds",
    "large_numbers",
    "chernick",
    "pomerance_lite",
    "pomerance_modular",
    "lambda_plus_one",
];
const BPSW_LARGE_START: &str = "1";
const BPSW_LARGE_ZEROS: usize = 300;

#[derive(Clone, Copy)]
struct BpswRangePreset {
    target_digits: i64,
    prime_digits: i64,
}

const BPSW_RANGE_PRESETS: [BpswRangePreset; 3] = [
    BpswRangePreset {
        target_digits: 22,
        prime_digits: 7,
    },
    BpswRangePreset {
        target_digits: 100,
        prime_digits: 18,
    },
    BpswRangePreset {
        target_digits: 1000,
        prime_digits: 19,
    },
];

fn app_version() -> String {
    let raw = include_str!("../../VERSION");
    raw.trim().to_string()
}

fn project_response(project: Project) -> crate::models::ProjectResponse {
    crate::models::ProjectResponse {
        id: project.id,
        guid: project.guid,
        name: project.name,
        description: project.description,
        owner_id: project.owner_id,
        status: project.status,
        is_demo: project.is_demo,
        storage_prefix: project.storage_prefix,
        created_at: project.created_at,
    }
}

pub struct ServiceError {
    pub status: StatusCode,
    pub body: ErrorResponse,
}

impl ServiceError {
    pub fn new(status: StatusCode, code: &'static str, message: impl ToString) -> Self {
        Self {
            status,
            body: ErrorResponse {
                code,
                message: message.to_string(),
                reasons: Vec::new(),
            },
        }
    }

    pub fn with_reasons(
        status: StatusCode,
        code: &'static str,
        message: impl ToString,
        reasons: Vec<String>,
    ) -> Self {
        Self {
            status,
            body: ErrorResponse {
                code,
                message: message.to_string(),
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
    if let Ok((agent_uid, display_name)) = resolve_agent_uid(None, Some(node_id.as_str())) {
        let db = state.db.lock().await;
        let _ = db::upsert_agent(&*db, &agent_uid, &display_name, None, None, None).await;
    }
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
    let (agent_uid, display_name, record) =
        resolve_agent_record(state, payload.agent_uid.as_deref(), payload.node_id.as_deref())
            .await?;
    let (blocked, blocked_reason) = match record {
        Some(record) => (record.blocked, record.blocked_reason),
        None => (false, None),
    };
    if blocked {
        return Ok(TaskResponse {
            status: "blocked".to_string(),
            task_id: "".to_string(),
            policy_decision: "deny",
            granted_tasks: 0,
            reasons: Vec::new(),
            payload: None,
            project_id: None,
            blocked,
            blocked_reason,
        });
    }

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
        PolicyDecision::Denied { reasons } => Err(ServiceError::with_reasons(
            StatusCode::FORBIDDEN,
            "policy_denied",
            "request denied by policy".to_string(),
            reasons,
        )),
        PolicyDecision::Limited { granted_tasks, .. } => {
            let batch = request_task_batch(
                state,
                TaskBatchRequest {
                    agent_uid: Some(agent_uid.to_string()),
                    node_id: display_name,
                    requested_tasks: Some(granted_tasks),
                    proposal_source: payload.proposal_source.clone(),
                    project_id: payload.project_id,
                    allowed_task_types: payload.allowed_task_types.clone(),
                },
            )
            .await?;
            Ok(batch_to_single("limit", batch))
        }
        PolicyDecision::Allowed { .. } => {
            let batch = request_task_batch(
                state,
                TaskBatchRequest {
                    agent_uid: Some(agent_uid.to_string()),
                    node_id: display_name,
                    requested_tasks: Some(1),
                    proposal_source: payload.proposal_source.clone(),
                    project_id: payload.project_id,
                    allowed_task_types: payload.allowed_task_types.clone(),
                },
            )
            .await?;
            Ok(batch_to_single("allow", batch))
        }
    }
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

    if let Err(err) = db::update_project_status(&transaction, project.id, "active").await {
        tracing::error!(error = %err, "update project status failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

    if let Err(err) = db::update_project_status(&transaction, project.id, "active").await {
        tracing::error!(error = %err, "update project status failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }

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

pub async fn request_task_batch(
    state: &AppState,
    payload: TaskBatchRequest,
) -> Result<TaskBatchResponse, ServiceError> {
    let requested_tasks = payload.requested_tasks.unwrap_or(1);
    let source = ProposalSource::from_optional(payload.proposal_source.as_deref());
    let proposal = TaskRequestProposal {
        requested_tasks,
        source,
    };

    let decision = state.policy.evaluate_task_request(proposal);
    let (policy_decision, reasons, granted_tasks) = match decision {
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
        } => ("limit", reasons, granted_tasks),
        PolicyDecision::Allowed { reasons } => ("allow", reasons, requested_tasks),
    };

    let (agent_uid, display_name, record) =
        resolve_agent_record(state, payload.agent_uid.as_deref(), payload.node_id.as_deref())
            .await?;
    let mut db = state.db.lock().await;
    let agent = match record {
        Some(record) => record,
        None => db::upsert_agent(&*db, &agent_uid, &display_name, None, None, None)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?,
    };

    if agent.blocked {
        return Ok(TaskBatchResponse {
            status: "blocked".to_string(),
            policy_decision,
            granted_tasks: 0,
            reasons,
            tasks: Vec::new(),
            blocked: true,
            blocked_reason: agent.blocked_reason,
        });
    }

    let project = if let Some(project_id) = payload.project_id {
        db::select_project_by_id(&mut db, project_id)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
            .ok_or_else(|| {
                ServiceError::new(
                    StatusCode::NOT_FOUND,
                    "project_not_found",
                    "project not found".to_string(),
                )
            })?
    } else {
        ensure_demo_project(&mut db).await?
    };

    if project.status != "active" {
        return Ok(TaskBatchResponse {
            status: project.status.clone(),
            policy_decision,
            granted_tasks: 0,
            reasons: vec!["project_not_active".to_string()],
            tasks: Vec::new(),
            blocked: false,
            blocked_reason: None,
        });
    }

    let schema = schema_name_for_project(&mut db, &project).await?;
    let allowed_task_types = match payload.allowed_task_types {
        Some(types) if !types.is_empty() => Some(types),
        _ => db::fetch_agent_preferences(&mut db, agent.id, project.id)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?,
    };

    let tasks = fetch_tasks_batch(
        &mut db,
        schema.as_str(),
        project.id,
        allowed_task_types.as_ref(),
        granted_tasks,
    )
    .await?;

    notify_update(state);
    Ok(TaskBatchResponse {
        status: "ok".to_string(),
        policy_decision,
        granted_tasks,
        reasons,
        tasks,
        blocked: false,
        blocked_reason: None,
    })
}

pub async fn register_agent(
    state: &AppState,
    payload: crate::models::AgentRegisterRequest,
) -> Result<crate::models::AgentRegisterResponse, ServiceError> {
    let agent_uid = parse_agent_uid(payload.agent_uid.as_str())?;
    let limits = payload.limits;
    let db = state.db.lock().await;
    let agent = db::upsert_agent(
        &*db,
        &agent_uid,
        &payload.display_name,
        limits.as_ref().and_then(|value| value.cpu_percent),
        limits.as_ref().and_then(|value| value.gpu_percent),
        limits.as_ref().and_then(|value| value.ram_percent),
    )
    .await
    .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

    db::upsert_agent_hardware(&*db, agent.id, &payload.hardware)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

    if let Some(preferences) = payload.preferences {
        for preference in preferences {
            db::upsert_agent_preferences(
                &*db,
                agent.id,
                preference.project_id,
                preference.allowed_task_types.as_slice(),
            )
            .await
            .map_err(|err| {
                ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err)
            })?;
        }
    }

    Ok(crate::models::AgentRegisterResponse {
        status: "ok",
        blocked: agent.blocked,
        blocked_reason: agent.blocked_reason,
    })
}

pub async fn update_agent_metrics(
    state: &AppState,
    payload: crate::models::AgentMetricsRequest,
) -> Result<crate::models::HeartbeatResponse, ServiceError> {
    let agent_uid = parse_agent_uid(payload.agent_uid.as_str())?;
    let db = state.db.lock().await;
    let agent = db::select_agent(&*db, &agent_uid)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
        .ok_or_else(|| {
            ServiceError::new(
                StatusCode::BAD_REQUEST,
                "agent_not_registered",
                "agent must register before sending metrics".to_string(),
            )
        })?;

    db::insert_agent_metrics(&*db, agent.id, &payload.metrics)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    notify_update(state);
    Ok(HeartbeatResponse { status: "ok" })
}

pub async fn update_agent_preferences(
    state: &AppState,
    payload: crate::models::AgentPreferencesRequest,
) -> Result<crate::models::HeartbeatResponse, ServiceError> {
    let agent_uid = parse_agent_uid(payload.agent_uid.as_str())?;
    let db = state.db.lock().await;
    let agent = db::select_agent(&*db, &agent_uid)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
        .ok_or_else(|| {
            ServiceError::new(
                StatusCode::BAD_REQUEST,
                "agent_not_registered",
                "agent must register before updating preferences".to_string(),
            )
        })?;

    for preference in payload.preferences {
        db::upsert_agent_preferences(
            &*db,
            agent.id,
            preference.project_id,
            preference.allowed_task_types.as_slice(),
        )
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    }

    Ok(HeartbeatResponse { status: "ok" })
}

pub async fn list_projects(state: &AppState) -> Result<Vec<Project>, ServiceError> {
    let mut db = state.db.lock().await;
    db::list_projects(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))
}

pub async fn list_agents(state: &AppState) -> Result<Vec<crate::models::AgentInfo>, ServiceError> {
    let db = state.db.lock().await;
    db::list_agents(&*db)
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

    let guid = uuid::Uuid::new_v4();
    let storage_prefix = guid.to_string();

    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    let project = match db::insert_project(
        &transaction,
        &guid,
        &name,
        &description,
        &owner_id,
        false,
        &storage_prefix,
    )
    .await
    {
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

    let response = CreateProjectResponse {
        status: "ok",
        project: project_response(project),
    };

    if let Some(storage) = &state.storage {
        if let Err(err) = storage
            .ensure_project_prefix(response.project.storage_prefix.as_str())
            .await
        {
            tracing::warn!(error = %err, "minio prefix init failed");
        }
    }

    Ok(response)
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

    if project.is_demo {
        return Err(ServiceError::new(
            StatusCode::FORBIDDEN,
            "demo_project",
            "demo project cannot be deleted".to_string(),
        ));
    }

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

pub async fn start_project(
    state: &AppState,
    project_id: i64,
) -> Result<crate::models::ProjectControlResponse, ServiceError> {
    let db = state.db.lock().await;
    let project = db::update_project_status(&*db, project_id, "active")
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    Ok(crate::models::ProjectControlResponse {
        status: "ok",
        project: project_response(project),
        affected_tasks: None,
    })
}

pub async fn pause_project(
    state: &AppState,
    project_id: i64,
) -> Result<crate::models::ProjectControlResponse, ServiceError> {
    let db = state.db.lock().await;
    let project = db::update_project_status(&*db, project_id, "paused")
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    Ok(crate::models::ProjectControlResponse {
        status: "ok",
        project: project_response(project),
        affected_tasks: None,
    })
}

pub async fn stop_project(
    state: &AppState,
    project_id: i64,
) -> Result<crate::models::ProjectControlResponse, ServiceError> {
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
    let schema = schema_name_for_project(&mut db, &project).await?;
    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;
    let project = db::update_project_status(&transaction, project_id, "stopped")
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    let affected = transaction
        .execute(db::task_stop_sql(&schema).as_str(), &[])
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    if let Err(err) = transaction.commit().await {
        tracing::error!(error = %err, "commit failed");
        return Err(ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        ));
    }
    Ok(crate::models::ProjectControlResponse {
        status: "ok",
        project: project_response(project),
        affected_tasks: Some(affected as u64),
    })
}

pub async fn start_demo_wordcount(
    state: &AppState,
    params: DemoStartParams,
) -> Result<DemoStartResponse, ServiceError> {
    let parts = params.parts.unwrap_or(5).max(1);

    let mut db = state.db.lock().await;
    let project = ensure_demo_project(&mut db).await?;
    if let Some(storage) = &state.storage {
        if let Err(err) = storage
            .ensure_project_prefix(project.storage_prefix.as_str())
            .await
        {
            tracing::warn!(error = %err, "minio prefix init failed");
        }
    }
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
        .query_one(insert_sql.as_str(), &[&"group", &"demo_group", &parent_payload])
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
            .query_one(insert_sql.as_str(), &[&"queued", &"demo_wordcount", &payload])
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
        .query(
            format!(
                "SELECT status, COUNT(*) AS count FROM {}.tasks WHERE task_type = 'demo_wordcount' GROUP BY status",
                schema
            )
            .as_str(),
            &[],
        )
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
        .query(
            format!(
                "SELECT status, COUNT(*) AS count FROM {}.tasks WHERE task_type = 'demo_wordcount' GROUP BY status",
                schema
            )
            .as_str(),
            &[],
        )
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
        .query(
            format!(
                "SELECT r.result FROM {}.task_results r \
                JOIN {}.tasks t ON t.id = r.task_id \
                WHERE t.task_type = 'demo_wordcount'",
                schema, schema
            )
            .as_str(),
            &[],
        )
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

pub async fn sync_bpsw_scripts(
    state: &AppState,
) -> Result<crate::models::BpswScriptSyncResponse, ServiceError> {
    let storage = state.storage.as_ref().ok_or_else(|| {
        ServiceError::new(
            StatusCode::FAILED_DEPENDENCY,
            "storage_unavailable",
            "minio is not configured".to_string(),
        )
    })?;

    let script_path = std::env::var("BPSW_SCRIPT_PATH")
        .unwrap_or_else(|_| format!("scripts/bpsw/{BPSW_SCRIPT_FILENAME}"));
    let script_bytes = std::fs::read(script_path.as_str()).map_err(|err| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "script_read_failed",
            format!("script read failed: {err}"),
        )
    })?;
    let script_hash = hash_bytes(script_bytes.as_slice());

    let mut db = state.db.lock().await;
    let project = ensure_bpsw_project(&mut db).await?;
    let object_key = format!(
        "{}/scripts/{}",
        project.storage_prefix, BPSW_SCRIPT_FILENAME
    );

    storage
        .put_object(object_key.as_str(), script_bytes)
        .await
        .map_err(|err| {
            ServiceError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "script_upload_failed",
                err,
            )
        })?;

    for task_type in BPSW_TASK_TYPES.iter() {
        db::upsert_project_task_type(
            &*db,
            project.id,
            task_type,
            object_key.as_str(),
            script_hash.as_str(),
            Some("4.3.0"),
        )
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    }

    Ok(crate::models::BpswScriptSyncResponse {
        status: "ok",
        project_id: project.id,
        task_types: BPSW_TASK_TYPES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    })
}

pub async fn start_bpsw_project(
    state: &AppState,
    payload: crate::models::BpswStartRequest,
) -> Result<crate::models::BpswStartResponse, ServiceError> {
    let mut db = state.db.lock().await;
    let project = ensure_bpsw_project(&mut db).await?;
    let schema = schema_name_for_project(&mut db, &project).await?;
    let task_types = payload
        .task_types
        .unwrap_or_else(|| BPSW_TASK_TYPES.iter().map(|value| value.to_string()).collect());

    let script_records = db::fetch_task_types(&mut db, project.id)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    if script_records.is_empty() {
        return Err(ServiceError::new(
            StatusCode::PRECONDITION_FAILED,
            "scripts_not_synced",
            "sync bpsw scripts first".to_string(),
        ));
    }

    let chunk_size = payload.chunk_size.unwrap_or(10_000).max(1) as i64;
    let max_tasks = env_or("MAX_BPSW_TASKS", 50_000u64);
    let mut total_tasks = 0usize;
    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    for task_type in task_types.iter() {
        let record = script_records
            .iter()
            .find(|(kind, _, _)| kind == task_type)
            .ok_or_else(|| {
                ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "unknown_task_type",
                    format!("task type {task_type} not configured"),
                )
            })?;

        let (object_key, script_hash) = (record.1.as_str(), record.2.as_str());
        let script_url = if let Some(storage) = &state.storage {
            storage
                .presign_get(object_key, 3600)
                .await
                .map_err(|err| {
                    ServiceError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "script_presign_failed",
                        err,
                    )
                })?
        } else {
            return Err(ServiceError::new(
                StatusCode::FAILED_DEPENDENCY,
                "storage_unavailable",
                "minio is not configured".to_string(),
            ));
        };

        let global_start = parse_bpsw_bound(payload.start.as_ref());
        let global_end = parse_bpsw_bound(payload.end.as_ref());
        let default_step = if task_type == "main_odds" || task_type == "large_numbers" {
            2
        } else {
            1
        };

        let mut task_specs = Vec::new();
        if task_type == "main_odds" {
            let start = global_start.unwrap_or_else(default_main_odds_start);
            let end = global_end.unwrap_or_else(|| chunk_end_from(&start, chunk_size, default_step));
            task_specs.push((start, end, None));
        } else if task_type == "large_numbers" {
            let start = global_start.unwrap_or_else(default_large_numbers_start);
            let end = global_end.unwrap_or_else(|| chunk_end_from(&start, chunk_size, default_step));
            task_specs.push((start, end, None));
        } else if task_type == "chernick" {
            if let (Some(start), Some(end)) = (global_start, global_end) {
                task_specs.push((start, end, None));
            } else {
                for preset in BPSW_RANGE_PRESETS.iter() {
                    let k_start = chernick_k_start(preset.target_digits);
                    let k_end = chunk_end_from(&k_start, chunk_size, default_step);
                    task_specs.push((k_start, k_end, Some(*preset)));
                }
            }
        } else {
            if let (Some(start), Some(end)) = (global_start, global_end) {
                for preset in BPSW_RANGE_PRESETS.iter() {
                    task_specs.push((start.clone(), end.clone(), Some(*preset)));
                }
            } else {
                let seed_start = num_bigint::BigInt::from(1);
                let seed_end = chunk_end_from(&seed_start, chunk_size, default_step);
                for preset in BPSW_RANGE_PRESETS.iter() {
                    task_specs.push((seed_start.clone(), seed_end.clone(), Some(*preset)));
                }
            }
        }

        for (start, end, preset) in task_specs {
            let estimated = estimate_bpsw_chunks(&start, &end, chunk_size, default_step)
                .map_err(|err| {
                    ServiceError::new(StatusCode::BAD_REQUEST, "invalid_range", err)
                })?;
            if total_tasks as u64 + estimated > max_tasks {
                return Err(ServiceError::new(
                    StatusCode::BAD_REQUEST,
                    "task_limit",
                    format!(
                        "range produces {} tasks; increase chunk_size or lower range (limit {})",
                        estimated, max_tasks
                    ),
                ));
            }
            for (chunk_start, chunk_end) in chunk_ranges(&start, &end, chunk_size, default_step) {
                let mut args = vec![
                    "--task-type".to_string(),
                    task_type.to_string(),
                ];
                match task_type.as_str() {
                    "main_odds" | "large_numbers" | "chernick" => {
                        args.push("--start".to_string());
                        args.push(chunk_start.clone());
                        args.push("--end".to_string());
                        args.push(chunk_end.clone());
                        if task_type == "chernick" {
                            args.push("--require-prime-factors".to_string());
                        }
                    }
                    _ => {
                        let preset = preset.expect("preset expected for generator tasks");
                        args.push("--seed-start".to_string());
                        args.push(chunk_start.clone());
                        args.push("--seed-end".to_string());
                        args.push(chunk_end.clone());
                        args.push("--target-digits".to_string());
                        args.push(preset.target_digits.to_string());
                        args.push("--prime-digits".to_string());
                        args.push(preset.prime_digits.to_string());
                        args.push("--max-steps".to_string());
                        args.push("5000".to_string());
                        if task_type == "lambda_plus_one" {
                            args.push("--require-prime".to_string());
                        }
                    }
                }

                let task_payload = json!({
                    "kind": "script_ref",
                    "task_type": task_type,
                    "script_url": script_url,
                    "script_sha256": script_hash,
                    "args": args
                });
                transaction
                    .execute(
                        db::task_insert_sql(&schema).as_str(),
                        &[&"queued", &task_type, &task_payload],
                    )
                    .await
                    .map_err(|err| {
                        ServiceError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "db_error",
                            format!("insert task failed: {err}"),
                        )
                    })?;
                total_tasks += 1;
            }
        }
    }

    transaction.commit().await.map_err(|err| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            format!("commit failed: {err}"),
        )
    })?;

    notify_update(state);
    Ok(crate::models::BpswStartResponse {
        status: "ok",
        project_id: project.id,
        total_tasks,
    })
}

fn estimate_bpsw_chunks(
    start: &BigInt,
    end: &BigInt,
    chunk_size: i64,
    step: i64,
) -> Result<u64, String> {
    if chunk_size <= 0 || step <= 0 {
        return Err("chunk_size and step must be positive".to_string());
    }
    if start > end {
        return Err("start must be <= end".to_string());
    }
    let span = end - start;
    let step_big = BigInt::from(step);
    let chunk_big = BigInt::from(chunk_size) * step_big;
    let mut chunks = &span / chunk_big.clone();
    if &span % chunk_big != BigInt::from(0) {
        chunks += 1;
    }
    let chunks_u64 = chunks
        .to_u64()
        .ok_or_else(|| "range too large".to_string())?;
    Ok(chunks_u64.max(1))
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
    let mut bucket_totals: Vec<DashboardPoint> = Vec::new();
    let mut completed_last_hour = 0u64;
    let bucket_hours = 4;
    let bucket_count = 7;

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
        completed_last_hour += db::task_completed_last_hours(&mut db, &schema, 1)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

        if tasks.is_empty() {
            tasks = db::fetch_recent_tasks(&mut db, &schema)
                .await
                .map_err(|err| {
                    ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err)
                })?;
        }

        let buckets = db::task_completed_buckets(&mut db, &schema, bucket_hours, bucket_count)
            .await
            .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

        if bucket_totals.is_empty() {
            bucket_totals = buckets;
        } else {
            for (idx, point) in bucket_totals.iter_mut().enumerate() {
                if let Some(bucket) = buckets.get(idx) {
                    point.value += bucket.value;
                }
            }
        }
    }

    if bucket_totals.is_empty() {
        bucket_totals = (0..bucket_count)
            .map(|idx| DashboardPoint {
                label: format!("-{}h", (bucket_count - 1 - idx) * bucket_hours),
                value: 0,
            })
            .collect();
    }

    let tasks_total_24h = bucket_totals.iter().map(|point| point.value).sum();
    let agent_availability = db::agent_availability_snapshot(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    let trust = db::trust_snapshot(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    let storage_io = db::storage_io_snapshot(&mut db)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;

    let ai_mode = state.policy.config().ai_mode.to_string();
    let updated_at = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let completed_last_min = load.completed_last_min;
    Ok(LiveSummary {
        updated_at,
        ai_mode,
        agents,
        tasks,
        queue,
        load,
        version: app_version(),
        dashboard: DashboardSnapshot {
            tasks_last_24h: bucket_totals,
            tasks_total_24h,
            agent_availability,
            storage_io,
            throughput: ThroughputSnapshot {
                completed_last_min,
                completed_last_hour,
            },
            trust,
        },
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

fn parse_bpsw_bound(bound: Option<&BpswBound>) -> Option<BigInt> {
    match bound {
        Some(BpswBound::Int(value)) => Some(BigInt::from(*value)),
        Some(BpswBound::Str(value)) => BigInt::parse_bytes(value.as_bytes(), 10),
        None => None,
    }
}

fn default_main_odds_start() -> BigInt {
    BigInt::parse_bytes(b"100000000000000000001", 10).unwrap_or_else(BigInt::zero)
}

fn default_large_numbers_start() -> BigInt {
    let mut value = String::from(BPSW_LARGE_START);
    value.push_str(&"0".repeat(BPSW_LARGE_ZEROS));
    value.push('1');
    BigInt::parse_bytes(value.as_bytes(), 10).unwrap_or_else(BigInt::zero)
}

fn pow10(exponent: u32) -> BigInt {
    let mut value = BigInt::one();
    for _ in 0..exponent {
        value *= 10u8;
    }
    value
}

fn chernick_k_start(target_digits: i64) -> BigInt {
    let digits = ((target_digits - 4) / 3).max(1) as u32;
    pow10(digits.saturating_sub(1))
}

fn chunk_end_from(start: &BigInt, chunk_size: i64, step: i64) -> BigInt {
    if chunk_size <= 1 {
        return start.clone();
    }
    let step = BigInt::from(step);
    let count = BigInt::from(chunk_size - 1);
    start + step * count
}

fn chunk_ranges(
    start: &BigInt,
    end: &BigInt,
    chunk_size: i64,
    step: i64,
) -> Vec<(String, String)> {
    let mut ranges = Vec::new();
    let step_big = BigInt::from(step);
    let mut current = start.clone();
    while &current <= end {
        let chunk_end = chunk_end_from(&current, chunk_size, step);
        let clamped = if &chunk_end > end {
            end.clone()
        } else {
            chunk_end
        };
        ranges.push((current.to_string(), clamped.to_string()));
        current = clamped + step_big.clone();
    }
    ranges
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

    let guid = uuid::Uuid::new_v4();
    let storage_prefix = guid.to_string();

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
        &guid,
        DEMO_PROJECT_NAME,
        &Some("Wordcount demo project".to_string()),
        &Option::<i64>::None,
        true,
        &storage_prefix,
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

async fn ensure_bpsw_project(db: &mut tokio_postgres::Client) -> Result<Project, ServiceError> {
    if let Some(project) = db::select_project_by_name(db, BPSW_PROJECT_NAME)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?
    {
        return Ok(project);
    }

    let guid = uuid::Uuid::new_v4();
    let storage_prefix = guid.to_string();
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
        &guid,
        BPSW_PROJECT_NAME,
        &Some("BPSW Hunter project".to_string()),
        &Option::<i64>::None,
        false,
        &storage_prefix,
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

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
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

    db.execute(
        db::task_insert_sql(schema).as_str(),
        &[&"queued", &"followup_report", &payload],
    )
        .await
        .map_err(|err| format!("insert followup failed: {err}"))?;
    Ok(())
}

fn batch_to_single(policy_decision: &'static str, batch: TaskBatchResponse) -> TaskResponse {
    let first = batch.tasks.first();
    TaskResponse {
        status: batch.status,
        task_id: first
            .map(|task| task.task_id.clone())
            .unwrap_or_else(|| "".to_string()),
        policy_decision,
        granted_tasks: batch.granted_tasks,
        reasons: batch.reasons,
        payload: first.map(|task| task.payload.clone()),
        project_id: first.map(|task| task.project_id),
        blocked: batch.blocked,
        blocked_reason: batch.blocked_reason,
    }
}

async fn resolve_agent_record(
    state: &AppState,
    agent_uid: Option<&str>,
    node_id: Option<&str>,
) -> Result<(Uuid, Option<String>, Option<db::AgentRecord>), ServiceError> {
    let (agent_uid, display_name) = resolve_agent_uid(agent_uid, node_id)?;
    let db = state.db.lock().await;
    let agent = db::select_agent(&*db, &agent_uid)
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    Ok((agent_uid, display_name, agent))
}

fn resolve_agent_uid(
    agent_uid: Option<&str>,
    node_id: Option<&str>,
) -> Result<(Uuid, Option<String>), ServiceError> {
    if let Some(agent_uid) = agent_uid {
        return Ok((parse_agent_uid(agent_uid)?, None));
    }
    if let Some(node_id) = node_id {
        if let Ok(parsed) = Uuid::parse_str(node_id) {
            return Ok((parsed, Some(node_id.to_string())));
        }
        return Ok((
            Uuid::new_v5(&Uuid::NAMESPACE_URL, node_id.as_bytes()),
            Some(node_id.to_string()),
        ));
    }
    Err(ServiceError::new(
        StatusCode::BAD_REQUEST,
        "missing_agent_uid",
        "agent_uid or node_id is required".to_string(),
    ))
}

fn parse_agent_uid(value: &str) -> Result<Uuid, ServiceError> {
    Uuid::parse_str(value).map_err(|_| {
        ServiceError::new(
            StatusCode::BAD_REQUEST,
            "invalid_agent_uid",
            "agent_uid must be a UUID".to_string(),
        )
    })
}

async fn fetch_tasks_batch(
    db: &mut tokio_postgres::Client,
    schema: &str,
    project_id: i64,
    allowed_task_types: Option<&Vec<String>>,
    limit: u32,
) -> Result<Vec<TaskAssignment>, ServiceError> {
    let transaction = db.transaction().await.map_err(|_| {
        ServiceError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "db_error",
            "database error".to_string(),
        )
    })?;

    let mut tasks = Vec::new();
    let limit = limit.max(1) as i64;
    if let Some(allowed) = allowed_task_types {
        let sql = format!(
            "SELECT id, payload, task_type FROM {}.tasks \
             WHERE status = 'queued' AND task_type = ANY($1) \
             ORDER BY id LIMIT $2 FOR UPDATE SKIP LOCKED",
            schema
        );
        let rows = transaction
            .query(sql.as_str(), &[allowed, &limit])
            .await
            .map_err(|err| {
                ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err)
            })?;
        for row in rows {
            tasks.push(TaskAssignment {
                task_id: row.get::<_, i64>("id").to_string(),
                payload: row.get("payload"),
                project_id,
                task_type: row.get("task_type"),
            });
        }
    } else {
        let sql = format!(
            "SELECT id, payload, task_type FROM {}.tasks \
             WHERE status = 'queued' \
             ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED",
            schema
        );
        let rows = transaction
            .query(sql.as_str(), &[&limit])
            .await
            .map_err(|err| {
                ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err)
            })?;
        for row in rows {
            tasks.push(TaskAssignment {
                task_id: row.get::<_, i64>("id").to_string(),
                payload: row.get("payload"),
                project_id,
                task_type: row.get("task_type"),
            });
        }
    }

    if tasks.is_empty() {
        let _ = transaction.commit().await;
        return Ok(tasks);
    }

    let ids: Vec<i64> = tasks
        .iter()
        .filter_map(|task| task.task_id.parse::<i64>().ok())
        .collect();
    let update_sql = format!(
        "UPDATE {}.tasks SET status = 'running', updated_at = NOW() WHERE id = ANY($1)",
        schema
    );
    transaction
        .execute(update_sql.as_str(), &[&ids])
        .await
        .map_err(|err| ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err))?;
    transaction.commit().await.map_err(|err| {
        ServiceError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", err.to_string())
    })?;

    Ok(tasks)
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
