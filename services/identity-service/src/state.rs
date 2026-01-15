use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub device_registry: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    pub max_devices_per_user: usize,
}

impl AppState {
    pub fn new(max_devices_per_user: usize) -> Self {
        Self {
            device_registry: Arc::new(Mutex::new(HashMap::new())),
            max_devices_per_user,
        }
    }
}
