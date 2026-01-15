mod app;
mod db;
mod handlers;
mod models;
mod policy;
mod service;
mod state;

use newral_common::{bind_listener, env_or, init_tracing, shutdown_signal};
use tokio_postgres::NoTls;

use crate::policy::PolicyConfig;
use crate::state::AppState;

#[tokio::main]
async fn main() {
    init_tracing();

    let port = env_or("PORT", 8080u16);
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let stream_interval = env_or("LIVE_UPDATE_INTERVAL_MS", 5000u64);
    let heartbeat_ttl = env_or("HEARTBEAT_TTL_SECS", 30u64);

    let policy = crate::policy::PolicyEngine::new(PolicyConfig::from_env());
    let (db, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect db");
    tokio::spawn(async move {
        // Drive the connection in the background.
        if let Err(err) = connection.await {
            tracing::error!(error = %err, "database connection error");
        }
    });

    let (updates, _) = tokio::sync::broadcast::channel(32);
    let state = AppState {
        policy: std::sync::Arc::new(policy),
        db: std::sync::Arc::new(tokio::sync::Mutex::new(db)),
        heartbeats: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        updates,
        stream_interval: std::time::Duration::from_millis(stream_interval),
        heartbeat_ttl: std::time::Duration::from_secs(heartbeat_ttl),
    };

    let app = app::build_router(state);
    let listener = bind_listener(port).await;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}
