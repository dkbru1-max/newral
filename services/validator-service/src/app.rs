use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers::{aggregate, healthz, readyz, recheck, validate};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/validate", post(validate))
        .route("/v1/sandbox/recheck", post(recheck))
        .route("/v1/sandbox/aggregate", post(aggregate))
        .with_state(state)
}
