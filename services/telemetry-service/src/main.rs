mod app;
mod handlers;
mod models;

use newral_common::{bind_listener, env_or, init_tracing, shutdown_signal};

#[tokio::main]
async fn main() {
    let _guards = init_tracing("telemetry-service");

    let port = env_or("PORT", 8080u16);
    let app = app::build_router();
    let listener = bind_listener(port).await;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}
