use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::models::{
    DeviceRegisterRequest, DeviceRegisterResponse, ErrorResponse, LoginRequest, LoginResponse,
    RegisterRequest, RegisterResponse,
};
use crate::state::AppState;

pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

pub async fn readyz() -> StatusCode {
    StatusCode::OK
}

pub async fn register(Json(payload): Json<RegisterRequest>) -> Json<RegisterResponse> {
    let _ = payload.email.as_deref();
    let _ = payload.password.as_deref();
    Json(RegisterResponse { status: "ok" })
}

pub async fn login(Json(payload): Json<LoginRequest>) -> Json<LoginResponse> {
    let _ = payload.email.as_deref();
    let _ = payload.password.as_deref();
    Json(LoginResponse {
        status: "ok",
        token: "dev-token",
    })
}

pub async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<DeviceRegisterRequest>,
) -> impl IntoResponse {
    // Track device count per user in memory (persistent storage comes later).
    let mut registry = state.device_registry.lock().await;
    let devices = registry
        .entry(payload.user_id.clone())
        .or_insert_with(std::collections::HashSet::new);

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
