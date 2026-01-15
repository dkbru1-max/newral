use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers::{healthz, login, readyz, register, register_device};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/register", post(register))
        .route("/v1/login", post(login))
        .route("/v1/devices/register", post(register_device))
        .with_state(state)
}
