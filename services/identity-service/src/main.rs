use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    net::SocketAddr,
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[allow(dead_code)]
const SQL_INSERT_USER: &str =
    "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id";
#[allow(dead_code)]
const SQL_SELECT_USER_BY_EMAIL: &str = "SELECT id, password_hash FROM users WHERE email = $1";
#[allow(dead_code)]
const SQL_INSERT_DEVICE: &str =
    "INSERT INTO devices (user_id, device_uid, name) VALUES ($1, $2, $3) RETURNING id";

#[derive(Clone)]
struct AppState {
    device_registry: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    max_devices_per_user: usize,
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: Option<String>,
    password: Option<String>,
}

#[derive(Serialize)]
struct RegisterResponse {
    status: &'static str,
}

#[derive(Deserialize)]
struct LoginRequest {
    email: Option<String>,
    password: Option<String>,
}

#[derive(Serialize)]
struct LoginResponse {
    status: &'static str,
    token: &'static str,
}

#[derive(Deserialize)]
struct DeviceRegisterRequest {
    user_id: String,
    device_id: String,
}

#[derive(Serialize)]
struct DeviceRegisterResponse {
    status: &'static str,
    device_count: usize,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: &'static str,
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
    let max_devices_per_user = env::var("MAX_DEVICES_PER_USER")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);

    // In-memory registry enforces per-user device cap for MVP.
    let state = AppState {
        device_registry: Arc::new(Mutex::new(HashMap::new())),
        max_devices_per_user,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/register", post(register))
        .route("/v1/login", post(login))
        .route("/v1/devices/register", post(register_device))
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

async fn register(Json(_payload): Json<RegisterRequest>) -> Json<RegisterResponse> {
    Json(RegisterResponse { status: "ok" })
}

async fn login(Json(_payload): Json<LoginRequest>) -> Json<LoginResponse> {
    Json(LoginResponse {
        status: "ok",
        token: "dev-token",
    })
}

async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<DeviceRegisterRequest>,
) -> impl IntoResponse {
    // Track device count per user in memory (persistent storage comes later).
    let mut registry = state.device_registry.lock().await;
    let devices = registry
        .entry(payload.user_id.clone())
        .or_insert_with(HashSet::new);

    if !devices.contains(&payload.device_id) && devices.len() >= state.max_devices_per_user {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                code: "device_limit",
                message: "device limit reached",
            }),
        )
            .into_response();
    }

    devices.insert(payload.device_id);
    let response = DeviceRegisterResponse {
        status: "ok",
        device_count: devices.len(),
    };
    (StatusCode::OK, Json(response)).into_response()
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
