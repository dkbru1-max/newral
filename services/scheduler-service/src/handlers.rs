use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{sse::Event, sse::KeepAlive, sse::Sse, IntoResponse},
    Json,
};
use std::{convert::Infallible, time::Duration};

use crate::models::{
    AgentMetricsRequest, AgentPreferencesRequest, AgentRegisterRequest, CreateProjectRequest,
    BpswStartRequest, DemoStartParams, HeartbeatRequest, TaskBatchRequest, TaskRequest,
    TaskSubmitRequest,
};
use crate::service;
use crate::state::AppState;

pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

pub async fn readyz() -> StatusCode {
    StatusCode::OK
}

pub async fn summary(State(state): State<AppState>) -> impl IntoResponse {
    match service::build_live_summary(&state).await {
        Ok(summary) => (StatusCode::OK, Json(summary)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn stream(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut updates = state.updates.subscribe();
    let interval = state.stream_interval;

    let stream = async_stream::stream! {
        let mut ticker = tokio::time::interval(interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {},
                _ = updates.recv() => {},
            }

            match service::build_live_summary(&state).await {
                Ok(summary) => {
                    if let Ok(event) = Event::default().json_data(summary) {
                        yield Ok(event);
                    }
                }
                Err(err) => {
                    let fallback = serde_json::json!({ "error": err.body.message });
                    if let Ok(event) = Event::default().json_data(fallback) {
                        yield Ok(event);
                    }
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

pub async fn request_task(
    State(state): State<AppState>,
    Json(payload): Json<TaskRequest>,
) -> impl IntoResponse {
    match service::request_task(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn request_task_batch(
    State(state): State<AppState>,
    Json(payload): Json<TaskBatchRequest>,
) -> impl IntoResponse {
    match service::request_task_batch(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn submit_task(
    State(state): State<AppState>,
    Json(payload): Json<TaskSubmitRequest>,
) -> impl IntoResponse {
    match service::submit_task(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn register_agent(
    State(state): State<AppState>,
    Json(payload): Json<AgentRegisterRequest>,
) -> impl IntoResponse {
    match service::register_agent(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn update_agent_metrics(
    State(state): State<AppState>,
    Json(payload): Json<AgentMetricsRequest>,
) -> impl IntoResponse {
    match service::update_agent_metrics(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn update_agent_preferences(
    State(state): State<AppState>,
    Json(payload): Json<AgentPreferencesRequest>,
) -> impl IntoResponse {
    match service::update_agent_preferences(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn heartbeat(
    State(state): State<AppState>,
    Json(payload): Json<HeartbeatRequest>,
) -> impl IntoResponse {
    match service::heartbeat(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    match service::list_projects(&state).await {
        Ok(projects) => (StatusCode::OK, Json(projects)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn create_project(
    State(state): State<AppState>,
    Json(payload): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    match service::create_project(&state, payload).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<i64>,
) -> impl IntoResponse {
    match service::delete_project(&state, project_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn start_demo_wordcount(
    State(state): State<AppState>,
    Query(params): Query<DemoStartParams>,
) -> impl IntoResponse {
    match service::start_demo_wordcount(&state, params).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn start_bpsw_project(
    State(state): State<AppState>,
    Json(payload): Json<BpswStartRequest>,
) -> impl IntoResponse {
    match service::start_bpsw_project(&state, payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn sync_bpsw_scripts(State(state): State<AppState>) -> impl IntoResponse {
    match service::sync_bpsw_scripts(&state).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn status_demo_wordcount(State(state): State<AppState>) -> impl IntoResponse {
    match service::status_demo_wordcount(&state).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}

pub async fn result_demo_wordcount(State(state): State<AppState>) -> impl IntoResponse {
    match service::result_demo_wordcount(&state).await {
        Ok((status, response)) => (status, Json(response)).into_response(),
        Err(err) => (err.status, Json(err.body)).into_response(),
    }
}
