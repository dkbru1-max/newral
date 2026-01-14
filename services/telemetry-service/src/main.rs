use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[allow(dead_code)]
const SQL_INSERT_TELEMETRY_FLAG: &str =
    "INSERT INTO flags (device_id, reason, details) VALUES ($1, $2, $3)";

#[derive(Deserialize)]
struct TelemetryEventRequest {
    event_type: String,
    payload: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct TelemetryEventResponse {
    status: &'static str,
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

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/event", post(event));

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

async fn event(Json(_payload): Json<TelemetryEventRequest>) -> Json<TelemetryEventResponse> {
    // Placeholder for future telemetry ingestion.
    Json(TelemetryEventResponse { status: "ok" })
}

async fn shutdown_signal() {
    // Use ctrl-c and SIGTERM where available.
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("sigterm handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
