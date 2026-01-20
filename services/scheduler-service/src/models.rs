use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TaskRequest {
    pub node_id: Option<String>,
    pub agent_uid: Option<String>,
    pub requested_tasks: Option<u32>,
    pub proposal_source: Option<String>,
    pub project_id: Option<i64>,
    pub allowed_task_types: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub status: String,
    pub task_id: String,
    pub policy_decision: &'static str,
    pub granted_tasks: u32,
    pub reasons: Vec<String>,
    pub payload: Option<serde_json::Value>,
    pub project_id: Option<i64>,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct TaskBatchRequest {
    pub agent_uid: Option<String>,
    pub node_id: Option<String>,
    pub requested_tasks: Option<u32>,
    pub proposal_source: Option<String>,
    pub project_id: Option<i64>,
    pub allowed_task_types: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct TaskBatchResponse {
    pub status: String,
    pub policy_decision: &'static str,
    pub granted_tasks: u32,
    pub reasons: Vec<String>,
    pub tasks: Vec<TaskAssignment>,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Serialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub payload: serde_json::Value,
    pub project_id: i64,
    pub task_type: Option<String>,
}

#[derive(Deserialize)]
pub struct TaskSubmitRequest {
    pub task_id: String,
    pub result: Option<String>,
    pub project_id: Option<i64>,
    pub device_id: Option<i64>,
}

#[derive(Serialize)]
pub struct TaskSubmitResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct LiveSummary {
    pub updated_at: String,
    pub ai_mode: String,
    pub agents: Vec<AgentSummary>,
    pub tasks: Vec<TaskSummary>,
    pub queue: TaskQueueSummary,
    pub load: LoadSummary,
    pub version: String,
    pub dashboard: DashboardSnapshot,
}

#[derive(Default, Serialize)]
pub struct DashboardSnapshot {
    pub tasks_last_24h: Vec<DashboardPoint>,
    pub tasks_total_24h: u64,
    pub agent_availability: AgentAvailabilitySnapshot,
    pub storage_io: StorageIoSnapshot,
    pub throughput: ThroughputSnapshot,
    pub trust: TrustSnapshot,
}

#[derive(Serialize)]
pub struct DashboardPoint {
    pub label: String,
    pub value: u64,
}

#[derive(Default, Serialize)]
pub struct AgentAvailabilitySnapshot {
    pub online: u64,
    pub idle: u64,
    pub blocked: u64,
}

#[derive(Default, Serialize)]
pub struct StorageIoSnapshot {
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
    pub net_rx_mb: f64,
    pub net_tx_mb: f64,
}

#[derive(Default, Serialize)]
pub struct ThroughputSnapshot {
    pub completed_last_min: u64,
    pub completed_last_hour: u64,
}

#[derive(Default, Serialize)]
pub struct TrustSnapshot {
    pub blocked_agents: u64,
    pub total_agents: u64,
}

#[derive(Serialize)]
pub struct AgentSummary {
    pub id: String,
    pub status: String,
    pub last_seen_secs: u64,
    pub region: String,
    pub reputation: String,
}

#[derive(Serialize)]
pub struct AgentInfo {
    pub agent_uid: String,
    pub display_name: Option<String>,
    pub status: String,
    pub last_seen: Option<String>,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
    pub hardware: Option<serde_json::Value>,
    pub metrics: Option<AgentMetrics>,
}

#[derive(Serialize)]
pub struct TaskSummary {
    pub id: String,
    pub project: String,
    pub status: String,
    pub priority: String,
    pub updated_at: String,
}

#[derive(Default, Serialize)]
pub struct TaskQueueSummary {
    pub queued: u64,
    pub running: u64,
    pub completed: u64,
}

#[derive(Default, Serialize)]
pub struct LoadSummary {
    pub running: u64,
    pub queued: u64,
    pub completed_last_min: u64,
}

#[derive(Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct ResourceLimits {
    pub cpu_percent: Option<f32>,
    pub gpu_percent: Option<f32>,
    pub ram_percent: Option<f32>,
}

#[derive(Deserialize, Serialize)]
pub struct ProjectPreference {
    pub project_id: i64,
    pub allowed_task_types: Vec<String>,
}

#[derive(Deserialize)]
pub struct AgentRegisterRequest {
    pub agent_uid: String,
    pub display_name: Option<String>,
    pub hardware: serde_json::Value,
    pub limits: Option<ResourceLimits>,
    pub preferences: Option<Vec<ProjectPreference>>,
}

#[derive(Serialize)]
pub struct AgentRegisterResponse {
    pub status: &'static str,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Deserialize)]
pub struct AgentMetricsRequest {
    pub agent_uid: String,
    pub metrics: AgentMetrics,
    pub hardware: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct PortalLogRequest {
    pub level: Option<String>,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
pub struct AgentMetrics {
    pub cpu_load: Option<f32>,
    pub ram_used_mb: Option<f32>,
    pub ram_total_mb: Option<f32>,
    pub gpu_load: Option<f32>,
    pub gpu_mem_used_mb: Option<f32>,
    pub net_rx_bytes: Option<i64>,
    pub net_tx_bytes: Option<i64>,
    pub disk_read_bytes: Option<i64>,
    pub disk_write_bytes: Option<i64>,
}

#[derive(Deserialize)]
pub struct AgentPreferencesRequest {
    pub agent_uid: String,
    pub preferences: Vec<ProjectPreference>,
}

#[derive(Serialize)]
pub struct HeartbeatResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub code: &'static str,
    pub message: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub guid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Option<i64>,
    pub status: String,
    pub is_demo: bool,
    pub storage_prefix: String,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner_id: Option<i64>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: i64,
    pub guid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Option<i64>,
    pub status: String,
    pub is_demo: bool,
    pub storage_prefix: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    pub status: &'static str,
    pub project: ProjectResponse,
}

#[derive(Serialize)]
pub struct ProjectControlResponse {
    pub status: &'static str,
    pub project: ProjectResponse,
    pub affected_tasks: Option<u64>,
}

#[derive(Deserialize)]
pub struct DemoStartParams {
    pub parts: Option<usize>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum BpswBound {
    Int(i64),
    Str(String),
}

#[derive(Deserialize)]
pub struct BpswStartRequest {
    pub start: Option<BpswBound>,
    pub end: Option<BpswBound>,
    pub chunk_size: Option<i64>,
    pub task_types: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct BpswStartResponse {
    pub status: &'static str,
    pub project_id: i64,
    pub total_tasks: usize,
}

#[derive(Serialize)]
pub struct BpswScriptSyncResponse {
    pub status: &'static str,
    pub project_id: i64,
    pub task_types: Vec<String>,
}

#[derive(Serialize)]
pub struct DemoStartResponse {
    pub status: &'static str,
    pub project_id: i64,
    pub total_tasks: usize,
    pub group_id: String,
    pub parent_task_id: i64,
}

#[derive(Serialize)]
pub struct DemoStatusResponse {
    pub total: usize,
    pub completed: usize,
    pub running: usize,
    pub queued: usize,
}

#[derive(Serialize)]
pub struct DemoResultResponse {
    pub total: usize,
    pub completed: usize,
    pub top_words: Vec<WordCount>,
}

#[derive(Serialize, Clone)]
pub struct WordCount {
    pub word: String,
    pub count: u64,
}
