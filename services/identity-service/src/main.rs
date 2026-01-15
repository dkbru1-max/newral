mod app;
mod handlers;
mod models;
mod state;

use axum::Router;
use newral_common::{bind_listener, env_or, init_tracing, shutdown_signal};

use crate::state::AppState;

#[tokio::main]
async fn main() {
    init_tracing();

    let port = env_or("PORT", 8080u16);
    let max_devices_per_user = env_or("MAX_DEVICES_PER_USER", 5usize);

    // In-memory registry enforces per-user device cap for MVP.
    let state = AppState::new(max_devices_per_user);

    let app: Router = app::build_router(state);
    let listener = bind_listener(port).await;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}
