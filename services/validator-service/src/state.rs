use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_postgres::Client;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Client>>,
    pub sandbox: ServerSandboxConfig,
    pub ai_enabled: bool,
}

#[derive(Clone)]
pub struct ServerSandboxConfig {
    pub python_bin: String,
    pub timeout: Duration,
    pub workspace_limit_bytes: u64,
    pub stdout_limit_bytes: u64,
    pub stderr_limit_bytes: u64,
}
