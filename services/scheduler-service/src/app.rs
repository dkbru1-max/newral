use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::handlers::{
    create_project, delete_project, healthz, heartbeat, list_projects, readyz, request_task,
    result_demo_wordcount, start_demo_wordcount, status_demo_wordcount, stream, submit_task,
    summary,
};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/summary", get(summary))
        .route("/v1/stream", get(stream))
        .route("/v1/tasks/request", post(request_task))
        .route("/v1/tasks/submit", post(submit_task))
        .route("/v1/heartbeat", post(heartbeat))
        .route("/v1/projects", get(list_projects))
        .route("/v1/projects", post(create_project))
        .route("/v1/projects/:id", delete(delete_project))
        .route("/v1/demo/wordcount/start", post(start_demo_wordcount))
        .route("/v1/demo/wordcount/status", get(status_demo_wordcount))
        .route("/v1/demo/wordcount/result", get(result_demo_wordcount))
        .with_state(state)
}
