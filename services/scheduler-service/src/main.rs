mod policy;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, sync::Arc};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::policy::{PolicyDecision, PolicyEngine, PolicyConfig, ProposalSource, TaskRequestProposal};

#[derive(Clone)]
struct AppState {
    policy: Arc<PolicyEngine>,
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

#[derive(Deserialize)]
struct TaskRequest {
    node_id: Option<String>,
    requested_tasks: Option<u32>,
    proposal_source: Option<String>,
}

#[derive(Serialize)]
struct TaskResponse {
    status: &'static str,
    task_id: &'static str,
    policy_decision: &'static str,
    granted_tasks: u32,
    reasons: Vec<String>,
}

#[derive(Deserialize)]
struct TaskSubmitRequest {
    task_id: String,
    result: Option<String>,
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
    message: &'static str,
    reasons: Vec<String>,
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    // Load policy limits from env for deterministic enforcement.
    let policy_config = PolicyConfig::from_env();
    let policy = PolicyEngine::new(policy_config);
    let state = AppState {
        policy: Arc::new(policy),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/tasks/request", post(request_task))
        .route("/v1/tasks/submit", post(submit_task))
        .route("/v1/heartbeat", post(heartbeat))
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
                message: "request denied by policy",
                reasons,
            }),
        )
            .into_response(),
        PolicyDecision::Limited {
            granted_tasks,
            reasons,
        } => (
            StatusCode::OK,
            Json(TaskResponse {
                status: "ok",
                task_id: "task-0000",
                policy_decision: "limit",
                granted_tasks,
                reasons,
            }),
        )
            .into_response(),
        PolicyDecision::Allowed { reasons } => (
            StatusCode::OK,
            Json(TaskResponse {
                status: "ok",
                task_id: "task-0000",
                policy_decision: "allow",
                granted_tasks: requested_tasks,
                reasons,
            }),
        )
            .into_response(),
    }
}

async fn submit_task(Json(_payload): Json<TaskSubmitRequest>) -> Json<TaskSubmitResponse> {
    // Placeholder for task result handling.
    Json(TaskSubmitResponse { status: "ok" })
}

async fn heartbeat(Json(payload): Json<HeartbeatRequest>) -> Json<HeartbeatResponse> {
    // Heartbeats are logged for liveness tracking.
    tracing::info!(
        node_id = payload.node_id.as_deref().unwrap_or("unknown"),
        "heartbeat received"
    );
    Json(HeartbeatResponse { status: "ok" })
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
