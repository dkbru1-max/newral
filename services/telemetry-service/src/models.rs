use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TelemetryEventRequest {
    pub event_type: String,
    pub payload: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct TelemetryEventResponse {
    pub status: &'static str,
}
