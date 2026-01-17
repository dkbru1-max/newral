mod ai;
mod app;
mod db;
mod handlers;
mod logic;
mod models;
mod sandbox;
mod state;

use newral_common::{bind_listener, env_or, init_tracing, shutdown_signal};
use tokio_postgres::NoTls;

use crate::state::{AppState, ServerSandboxConfig};

#[tokio::main]
async fn main() {
    let _guards = init_tracing("validator-service");

    let port = env_or("PORT", 8080u16);
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL is required");

    let (db, connection) = tokio_postgres::connect(&database_url, NoTls)
        .await
        .expect("connect db");
    tokio::spawn(async move {
        // Drive the connection in the background.
        if let Err(err) = connection.await {
            tracing::error!(error = %err, "database connection error");
        }
    });

    let state = AppState {
        db: std::sync::Arc::new(tokio::sync::Mutex::new(db)),
        sandbox: ServerSandboxConfig {
            python_bin: std::env::var("SANDBOX_SERVER_PYTHON_BIN")
                .unwrap_or_else(|_| "python3".to_string()),
            timeout: std::time::Duration::from_secs(env_or("SANDBOX_SERVER_TIMEOUT_SECS", 20u64)),
            workspace_limit_bytes: env_or("SANDBOX_SERVER_WORKSPACE_LIMIT_MB", 128u64)
                * 1024
                * 1024,
            stdout_limit_bytes: env_or("SANDBOX_SERVER_STDOUT_LIMIT_MB", 2u64) * 1024 * 1024,
            stderr_limit_bytes: env_or("SANDBOX_SERVER_STDERR_LIMIT_MB", 2u64) * 1024 * 1024,
        },
        ai_enabled: std::env::var("AI_VALIDATE")
            .ok()
            .map(|value| value != "0")
            .unwrap_or(true),
    };

    let app = app::build_router(state);
    let listener = bind_listener(port).await;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve");
}
