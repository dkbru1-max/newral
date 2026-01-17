use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct ValidateRequest {
    pub task_id: i64,
    pub device_id: i64,
    pub result_hash: Option<String>,
    pub outcome: Option<String>,
}

#[derive(Serialize)]
pub struct ValidateResponse {
    pub status: &'static str,
    pub decision: &'static str,
    pub reputation_score: f64,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Deserialize)]
pub struct SandboxRecheckRequest {
    pub project_id: i64,
    pub task_id: i64,
    pub device_id: Option<i64>,
}

#[derive(Serialize)]
pub struct SandboxRecheckResponse {
    pub status: &'static str,
    pub decision: &'static str,
    pub ai_flag: Option<String>,
    pub server_result: serde_json::Value,
    pub agent_result: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct SandboxAggregateRequest {
    pub project_id: i64,
    pub group_id: String,
}

#[derive(Serialize)]
pub struct SandboxAggregateResponse {
    pub status: &'static str,
    pub group_id: String,
    pub total: u64,
    pub completed: u64,
    pub aggregated: serde_json::Value,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct TaskPayload {
    pub kind: Option<String>,
    pub script: Option<String>,
    pub script_url: Option<String>,
    pub script_sha256: Option<String>,
    pub args: Option<Vec<String>>,
    pub inputs: Option<HashMap<String, String>>,
    pub group_id: Option<String>,
    pub parent_task_id: Option<i64>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct AgentResult {
    pub status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub duration_ms: Option<u64>,
    pub stdout_sha256: Option<String>,
    pub script_sha256: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ServerSandboxResult {
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub started_at_ms: u128,
    pub ended_at_ms: u128,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    pub stdout_sha256: String,
    pub script_sha256: String,
}
