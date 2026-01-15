use axum::{http::StatusCode, Json};

use crate::models::{TelemetryEventRequest, TelemetryEventResponse};

pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

pub async fn readyz() -> StatusCode {
    StatusCode::OK
}

pub async fn event(Json(payload): Json<TelemetryEventRequest>) -> Json<TelemetryEventResponse> {
    let _ = payload.event_type.as_str();
    let _ = payload.payload.as_ref();
    // Placeholder for future telemetry ingestion.
    Json(TelemetryEventResponse { status: "ok" })
}
