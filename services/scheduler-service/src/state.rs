use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex};
use tokio_postgres::Client;

use crate::policy::PolicyEngine;
use crate::storage::StorageClient;

#[derive(Clone)]
pub struct AppState {
    pub policy: Arc<PolicyEngine>,
    pub db: Arc<Mutex<Client>>,
    pub heartbeats: Arc<Mutex<HashMap<String, AgentHeartbeat>>>,
    pub updates: broadcast::Sender<()>,
    pub stream_interval: Duration,
    pub heartbeat_ttl: Duration,
    pub storage: Option<StorageClient>,
}

#[derive(Clone)]
pub struct AgentHeartbeat {
    pub last_seen: std::time::SystemTime,
}
