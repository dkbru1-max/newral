use axum::{
    routing::{get, post},
    Router,
};
use tower_http::trace::TraceLayer;

use crate::handlers::{event, healthz, readyz};

pub fn build_router() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/event", post(event))
        .layer(TraceLayer::new_for_http())
}
