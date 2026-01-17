use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::handlers::{
    create_project, delete_project, healthz, heartbeat, list_projects, readyz, register_agent,
    request_task, request_task_batch, result_demo_wordcount, start_demo_wordcount,
    start_bpsw_project, status_demo_wordcount, stream, submit_task, summary, sync_bpsw_scripts,
    update_agent_metrics, update_agent_preferences,
};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/summary", get(summary))
        .route("/v1/stream", get(stream))
        .route("/v1/tasks/request", post(request_task))
        .route("/v1/tasks/request_batch", post(request_task_batch))
        .route("/v1/tasks/submit", post(submit_task))
        .route("/v1/heartbeat", post(heartbeat))
        .route("/v1/agents/register", post(register_agent))
        .route("/v1/agents/metrics", post(update_agent_metrics))
        .route("/v1/agents/preferences", post(update_agent_preferences))
        .route("/v1/projects", get(list_projects))
        .route("/v1/projects", post(create_project))
        .route("/v1/projects/:id", delete(delete_project))
        .route("/v1/demo/wordcount/start", post(start_demo_wordcount))
        .route("/v1/demo/wordcount/status", get(status_demo_wordcount))
        .route("/v1/demo/wordcount/result", get(result_demo_wordcount))
        .route("/v1/projects/bpsw/start", post(start_bpsw_project))
        .route("/v1/projects/bpsw/scripts/sync", post(sync_bpsw_scripts))
        .with_state(state)
}
