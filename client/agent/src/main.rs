#![cfg_attr(feature = "gui", windows_subsystem = "windows")]

use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    env,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use sysinfo::{Disks, Networks, System};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{Mutex, Notify},
    time::sleep,
};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use uuid::Uuid;

const EULA_TEXT: &str = r#"End User License Agreement (EULA) for Newral Agent

Data Collection: By using the Newral Agent software ("the Software"), you acknowledge and consent that the Software will collect and periodically transmit technical information about your computer system to the Newral central server. This includes, but is not limited to, your IP address, CPU and GPU specifications, amount of RAM, disk capacity and usage, and other hardware metrics necessary for the operation of the distributed computing platform. No personal files or sensitive personal data will be accessed or transmitted.

Purpose of Data: The collected information is used to monitor node performance, ensure the integrity and efficiency of distributed computations, and improve the overall platform. It may also be used to calculate rewards or reputation scores for your contributions.

Privacy and Use of Data: All collected data is used solely within the Newral platform. It will not be shared with unauthorized third parties. The data is handled in accordance with applicable privacy laws and is used only for operational analytics, security verification, and platform research and improvements.

Software Updates: This Software may automatically download and install updates to improve its functionality and security. By using the Software, you agree that such updates are part of the Software's operation. The EULA terms may be updated with such releases, and continued use of the Software constitutes acceptance of the updated terms.

User Obligations: You agree to use the Software only for its intended purpose as part of the Newral distributed computing platform. You will not attempt to reverse engineer, tamper with, or misuse the Software or the data it processes.

Termination: You may stop using the Software at any time. The Newral team reserves the right to terminate your access to the platform if you violate the terms of this EULA.

Disclaimer of Warranty: The Software is provided "as is" without warranty of any kind. The Newral developers disclaim all warranties, express or implied, including but not limited to the warranties of merchantability and fitness for a particular purpose. You assume all risks associated with using the Software.

Limitation of Liability: In no event shall the Newral platform or its developers be liable for any damages or losses arising from the use of or inability to use the Software, even if advised of the possibility of such damages.

Acceptance: By clicking "Accept" and using the Software, you indicate that you have read, understood, and agree to all the terms and conditions of this EULA. If you do not agree, do not use the Software.
"#;

const AGENT_VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Debug, Clone)]
struct AgentConfig {
    node_id: String,
    display_name: Option<String>,
    scheduler_url: String,
    heartbeat_interval: Duration,
    poll_interval: Duration,
    runner_sleep: Duration,
    batch_min: u32,
    batch_max: u32,
    batch_delay_min: Duration,
    batch_delay_max: Duration,
    metrics_interval: Duration,
    sandbox: SandboxConfig,
    eula_accepted: bool,
    project_id: Option<i64>,
    allowed_task_types: Vec<String>,
    limits: ResourceLimits,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
struct FileConfig {
    node_id: Option<String>,
    display_name: Option<String>,
    scheduler_url: Option<String>,
    heartbeat_interval_secs: Option<u64>,
    poll_interval_secs: Option<u64>,
    runner_sleep_secs: Option<u64>,
    batch_min: Option<u32>,
    batch_max: Option<u32>,
    batch_delay_min_secs: Option<u64>,
    batch_delay_max_secs: Option<u64>,
    metrics_interval_secs: Option<u64>,
    python_bin: Option<String>,
    timeout_secs: Option<u64>,
    workspace_limit_mb: Option<u64>,
    stdout_limit_mb: Option<u64>,
    stderr_limit_mb: Option<u64>,
    eula_accepted: Option<bool>,
    project_id: Option<i64>,
    allowed_task_types: Option<Vec<String>>,
    cpu_limit_percent: Option<f32>,
    gpu_limit_percent: Option<f32>,
    ram_limit_percent: Option<f32>,
}

#[derive(Clone)]
struct Agent {
    config: std::sync::Arc<AgentConfig>,
    client: Client,
    runner: std::sync::Arc<dyn TaskRunner + Send + Sync>,
    runtime: Option<AgentRuntime>,
}

#[derive(Serialize)]
struct HeartbeatRequest<'a> {
    node_id: &'a str,
}

#[derive(Serialize)]
struct TaskRequest<'a> {
    node_id: &'a str,
    agent_uid: &'a str,
    requested_tasks: u32,
    proposal_source: &'a str,
    project_id: Option<i64>,
    allowed_task_types: Vec<String>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
struct TaskResponse {
    status: String,
    task_id: String,
    policy_decision: String,
    granted_tasks: u32,
    reasons: Vec<String>,
    payload: Option<TaskPayload>,
    project_id: Option<i64>,
    blocked: Option<bool>,
    blocked_reason: Option<String>,
}

#[derive(Serialize)]
struct TaskBatchRequest<'a> {
    agent_uid: &'a str,
    requested_tasks: u32,
    proposal_source: &'a str,
    project_id: Option<i64>,
    allowed_task_types: Vec<String>,
}

#[derive(Deserialize, Default)]
struct TaskBatchResponse {
    status: String,
    policy_decision: String,
    granted_tasks: u32,
    reasons: Vec<String>,
    tasks: Vec<TaskAssignment>,
    blocked: Option<bool>,
    blocked_reason: Option<String>,
}

#[derive(Serialize)]
struct TaskSubmitRequest<'a> {
    task_id: &'a str,
    result: &'a str,
    project_id: Option<i64>,
    device_id: Option<i64>,
}

#[derive(Deserialize)]
struct TaskSubmitResponse {
    status: String,
}

#[derive(Deserialize)]
struct TaskAssignment {
    task_id: String,
    payload: TaskPayload,
    project_id: i64,
    task_type: Option<String>,
}

#[derive(Debug)]
struct Task {
    id: String,
    payload: TaskPayload,
    project_id: Option<i64>,
    task_type: Option<String>,
}

#[derive(Serialize)]
struct AgentRegisterRequest<'a> {
    agent_uid: &'a str,
    display_name: Option<&'a str>,
    hardware: serde_json::Value,
    limits: ResourceLimits,
    preferences: Vec<ProjectPreference>,
}

#[derive(Deserialize)]
struct AgentRegisterResponse {
    status: String,
    blocked: bool,
    blocked_reason: Option<String>,
}

#[derive(Serialize)]
struct AgentMetricsRequest<'a> {
    agent_uid: &'a str,
    metrics: AgentMetrics,
    hardware: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct AgentMetrics {
    cpu_load: Option<f32>,
    ram_used_mb: Option<f32>,
    ram_total_mb: Option<f32>,
    gpu_load: Option<f32>,
    gpu_mem_used_mb: Option<f32>,
    net_rx_bytes: Option<i64>,
    net_tx_bytes: Option<i64>,
    disk_read_bytes: Option<i64>,
    disk_write_bytes: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ProjectPreference {
    project_id: i64,
    allowed_task_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TaskPayload {
    kind: Option<String>,
    script: Option<String>,
    script_url: Option<String>,
    script_sha256: Option<String>,
    args: Option<Vec<String>>,
    inputs: Option<HashMap<String, String>>,
    task_type: Option<String>,
}

#[derive(Debug, Clone)]
struct SandboxConfig {
    python_bin: String,
    timeout: Duration,
    workspace_limit_bytes: u64,
    stdout_limit_bytes: u64,
    stderr_limit_bytes: u64,
}

#[derive(Serialize)]
struct SandboxResult {
    status: String,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    started_at_ms: u128,
    ended_at_ms: u128,
    exit_code: Option<i32>,
    error: Option<String>,
    stdout_bytes: u64,
    stderr_bytes: u64,
    stdout_sha256: String,
    script_sha256: Option<String>,
    workspace_bytes: u64,
    files_written: u64,
    engine: String,
    node_id: String,
    task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ResourceLimits {
    cpu_percent: Option<f32>,
    gpu_percent: Option<f32>,
    ram_percent: Option<f32>,
}

impl ResourceLimits {
    fn any_set(&self) -> bool {
        self.cpu_percent.is_some() || self.gpu_percent.is_some() || self.ram_percent.is_some()
    }
}

#[derive(Clone)]
struct StopSignal {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl StopSignal {
    fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    fn stop(&self) {
        self.flag.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    fn stopped(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    async fn sleep_or_stop(&self, duration: Duration) -> bool {
        if self.stopped() {
            return true;
        }
        tokio::select! {
            _ = sleep(duration) => false,
            _ = self.notify.notified() => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogLevel {
    Info,
    Warn,
    Error,
    Success,
}

impl LogLevel {
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
            LogLevel::Success => "success",
        }
    }
}

#[derive(Clone)]
#[cfg_attr(not(feature = "gui"), allow(dead_code))]
struct LogEntry {
    level: LogLevel,
    message: String,
}

#[derive(Clone)]
struct LogBuffer {
    lines: Arc<StdMutex<VecDeque<LogEntry>>>,
    limit: usize,
}

impl LogBuffer {
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    fn new(limit: usize) -> Self {
        Self {
            lines: Arc::new(StdMutex::new(VecDeque::new())),
            limit,
        }
    }

    fn push_entry(&self, level: LogLevel, message: String) {
        let mut lines = self.lines.lock().unwrap();
        lines.push_back(LogEntry { level, message });
        while lines.len() > self.limit {
            lines.pop_front();
        }
    }

    fn push_line(&self, level: LogLevel, line: &str) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            self.push_entry(level, trimmed.to_string());
        }
    }

    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    fn snapshot(&self) -> Vec<LogEntry> {
        self.lines.lock().unwrap().iter().cloned().collect()
    }
}

struct LogWriter {
    buffer: LogBuffer,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let line = String::from_utf8_lossy(buf).to_string();
        self.buffer.push_line(LogLevel::Info, line.as_str());
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogBuffer {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.clone(),
        }
    }
}

#[derive(Default, Clone)]
struct AgentRuntimeState {
    connected: bool,
    paused: bool,
    blocked: bool,
    blocked_reason: Option<String>,
    current_task: Option<String>,
    last_result: Option<String>,
    last_error: Option<String>,
}

#[derive(Clone)]
struct AgentRuntime {
    status: Arc<Mutex<AgentRuntimeState>>,
    logs: LogBuffer,
}

impl AgentRuntime {
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    fn new(logs: LogBuffer) -> Self {
        Self {
            status: Arc::new(Mutex::new(AgentRuntimeState::default())),
            logs,
        }
    }

    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    fn with_state(logs: LogBuffer, status: Arc<Mutex<AgentRuntimeState>>) -> Self {
        Self { status, logs }
    }

    fn log(&self, level: LogLevel, message: &str) {
        self.logs.push_line(level, message);
    }
}

fn init_tracing(log_buffer: Option<LogBuffer>) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_ansi(false);
    let writer = match log_buffer {
        Some(buffer) => tracing_subscriber::fmt::writer::BoxMakeWriter::new(buffer),
        None => tracing_subscriber::fmt::writer::BoxMakeWriter::new(io::stdout),
    };
    let subscriber = builder.with_writer(writer).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
fn should_run_service() -> bool {
    env::args().any(|arg| arg == "--service")
}

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
fn run_service_mode() {
    init_tracing(None);
    let config = match load_config() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "failed to load config");
            return;
        }
    };
    let agent = Agent {
        config: std::sync::Arc::new(config),
        client: Client::new(),
        runner: std::sync::Arc::new(SandboxRunner {}),
        runtime: None,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(run_agent_until_stop(agent));
}

async fn run_agent_until_stop(agent: Agent) {
    if !agent.config.eula_accepted {
        tracing::error!("EULA not accepted; exiting");
        agent.log(LogLevel::Error, "EULA not accepted. Please accept in settings.");
        return;
    }

    match agent.register().await {
        Ok(response) => {
            if response.blocked {
                agent.log(LogLevel::Error, "Agent is blocked by server");
                agent
                    .set_blocked(true, response.blocked_reason)
                    .await;
                return;
            }
        }
        Err(err) => {
            tracing::error!(error = %err, "registration failed");
            agent.log(LogLevel::Error, "Agent registration failed");
            return;
        }
    }

    let stop = StopSignal::new();
    let handles = spawn_agent(agent, stop.clone());

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown requested");
            stop.stop();
        }
    }

    for handle in handles {
        let _ = handle.await;
    }
}

#[cfg(feature = "gui")]
fn main() {
    if should_run_service() {
        run_service_mode();
        return;
    }
    gui::run();
}

#[cfg(not(feature = "gui"))]
#[tokio::main]
async fn main() {
    init_tracing(None);

    // Load config from file and env; env wins.
    let config = match load_config() {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(error = %err, "failed to load config");
            return;
        }
    };

    let agent = Agent {
        config: std::sync::Arc::new(config),
        client: Client::new(),
        runner: std::sync::Arc::new(SandboxRunner {}),
        runtime: None,
    };

    run_agent_until_stop(agent).await;
}

impl Agent {
    fn log(&self, level: LogLevel, message: &str) {
        if let Some(runtime) = &self.runtime {
            runtime.log(level, message);
        }
    }

    async fn set_connected(&self, connected: bool) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.connected = connected;
        }
    }

    async fn set_paused(&self, paused: bool) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.paused = paused;
        }
    }

    async fn set_blocked(&self, blocked: bool, reason: Option<String>) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.blocked = blocked;
            status.blocked_reason = reason;
        }
    }

    async fn is_paused(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            let status = runtime.status.lock().await;
            return status.paused;
        }
        false
    }

    async fn is_blocked(&self) -> bool {
        if let Some(runtime) = &self.runtime {
            let status = runtime.status.lock().await;
            return status.blocked;
        }
        false
    }

    async fn set_current_task(&self, task_id: Option<String>) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.current_task = task_id;
        }
    }

    async fn set_last_result(&self, result: Option<String>) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.last_result = result;
        }
    }

    async fn set_last_error(&self, message: Option<String>) {
        if let Some(runtime) = &self.runtime {
            let mut status = runtime.status.lock().await;
            status.last_error = message;
        }
    }

    async fn heartbeat_loop(self, stop: StopSignal) {
        loop {
            if stop.stopped() {
                break;
            }
            if let Err(err) = self.send_heartbeat().await {
                tracing::warn!(error = %err, "heartbeat failed");
                self.log(LogLevel::Warn, "Heartbeat failed");
                self.set_connected(false).await;
                self.set_last_error(Some(format!("heartbeat: {err}"))).await;
            }
            if stop.sleep_or_stop(self.config.heartbeat_interval).await {
                break;
            }
        }
    }

    async fn send_heartbeat(&self) -> Result<(), reqwest::Error> {
        // Heartbeat endpoint is stubbed in scheduler-service.
        let url = format!("{}/v1/heartbeat", self.config.scheduler_url);
        let response = self
            .client
            .post(url)
            .json(&HeartbeatRequest {
                node_id: &self.config.node_id,
            })
            .send()
            .await?;

        tracing::info!(status = response.status().as_u16(), "heartbeat sent");
        if response.status().is_success() {
            self.log(LogLevel::Info, "Heartbeat ok");
            self.set_connected(true).await;
            self.set_last_error(None).await;
        }
        Ok(())
    }

    async fn register(&self) -> Result<AgentRegisterResponse, reqwest::Error> {
        let url = format!("{}/v1/agents/register", self.config.scheduler_url);
        let hardware = collect_hardware_info();
        let preferences = if let Some(project_id) = self.config.project_id {
            vec![ProjectPreference {
                project_id,
                allowed_task_types: self.config.allowed_task_types.clone(),
            }]
        } else {
            Vec::new()
        };
        let response = self
            .client
            .post(url)
            .json(&AgentRegisterRequest {
                agent_uid: &self.config.node_id,
                display_name: self.config.display_name.as_deref(),
                hardware,
                limits: self.config.limits.clone(),
                preferences,
            })
            .send()
            .await?;
        response.json().await
    }

    async fn metrics_loop(&self, stop: StopSignal) {
        let mut system = System::new_all();
        let mut sent_hardware = false;
        loop {
            if stop.stopped() {
                break;
            }
            let metrics = collect_metrics(&mut system);
            let url = format!("{}/v1/agents/metrics", self.config.scheduler_url);
            let hardware = if sent_hardware {
                None
            } else {
                Some(collect_hardware_info())
            };
            let response = self
                .client
                .post(url)
                .json(&AgentMetricsRequest {
                    agent_uid: &self.config.node_id,
                    metrics,
                    hardware,
                })
                .send()
                .await;
            match response {
                Ok(response) => {
                    if response.status().is_success() {
                        sent_hardware = true;
                    } else if !sent_hardware {
                        tracing::warn!(
                            status = response.status().as_u16(),
                            "metrics upload rejected"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "metrics send failed");
                }
            }
            if stop.sleep_or_stop(self.config.metrics_interval).await {
                break;
            }
        }
    }

    async fn throttle_until_within_limits(&self, stop: StopSignal) {
        if !self.config.limits.any_set() {
            return;
        }
        let mut system = System::new_all();
        let mut warned_gpu = false;
        loop {
            if stop.stopped() || self.is_paused().await {
                break;
            }
            let metrics = collect_metrics(&mut system);
            if self.config.limits.gpu_percent.is_some() && metrics.gpu_load.is_none() && !warned_gpu
            {
                self.log(
                    LogLevel::Warn,
                    "GPU limit configured but GPU metrics unavailable",
                );
                warned_gpu = true;
            }
            let exceeded = exceeded_limits(&metrics, &self.config.limits);
            if exceeded.is_empty() {
                break;
            }
            self.log(
                LogLevel::Warn,
                &format!("Resource limits reached: {}", exceeded.join(", ")),
            );
            if stop.sleep_or_stop(Duration::from_secs(2)).await {
                break;
            }
        }
    }

    async fn run_loop(&self, stop: StopSignal) {
        let mut queue: VecDeque<Task> = VecDeque::new();
        loop {
            if stop.stopped() {
                break;
            }
            if self.is_blocked().await {
                self.log(LogLevel::Error, "Agent blocked by server");
                break;
            }
            if self.is_paused().await {
                if stop.sleep_or_stop(Duration::from_secs(2)).await {
                    break;
                }
                continue;
            }

            if queue.is_empty() {
                match self.request_tasks_batch().await {
                    Ok(Some(tasks)) => {
                        for task in tasks {
                            queue.push_back(task);
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("no tasks available");
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "task request failed");
                        self.log(LogLevel::Warn, "Task request failed");
                        self.set_last_error(Some(format!("request: {err}"))).await;
                    }
                }
            }

            if let Some(task) = queue.pop_front() {
                self.throttle_until_within_limits(stop.clone()).await;
                self.set_current_task(Some(task.id.clone())).await;
                self.log(LogLevel::Info, &format!("Task start {}", task.id));
                // Runner abstracts future sandboxed execution.
                let result = self
                    .runner
                    .run(
                        &task,
                        &self.config.sandbox,
                        &self.config.limits,
                        self.config.node_id.as_str(),
                        self.config.runner_sleep,
                    )
                    .await;
                if result.starts_with("error:") {
                    self.log(LogLevel::Error, &format!("Task failed {}", task.id));
                    self.set_last_error(Some(result.clone())).await;
                } else {
                    self.log(LogLevel::Success, &format!("Task done {}", task.id));
                }
                if let Err(err) = self.submit_result(&task, &result).await {
                    tracing::warn!(error = %err, "submit failed");
                    self.log(LogLevel::Warn, "Result submit failed");
                    self.set_last_error(Some(format!("submit: {err}"))).await;
                } else {
                    self.set_last_result(Some(result.clone())).await;
                }
                self.set_current_task(None).await;

                if queue.is_empty() {
                    let min_delay = self.config.batch_delay_min.as_secs();
                    let max_delay = self.config.batch_delay_max.as_secs();
                    let delay = if min_delay >= max_delay {
                        min_delay
                    } else {
                        rand::thread_rng().gen_range(min_delay..=max_delay)
                    };
                    if stop.sleep_or_stop(Duration::from_secs(delay)).await {
                        break;
                    }
                }
            } else if stop.sleep_or_stop(self.config.poll_interval).await {
                break;
            }
        }
    }

    async fn request_task(&self) -> Result<Option<Task>, reqwest::Error> {
        let url = format!("{}/v1/tasks/request", self.config.scheduler_url);
        let response = self
            .client
            .post(url)
            .json(&TaskRequest {
                node_id: &self.config.node_id,
                agent_uid: &self.config.node_id,
                requested_tasks: 1,
                proposal_source: "system",
                project_id: self.config.project_id,
                allowed_task_types: self.config.allowed_task_types.clone(),
            })
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                "scheduler rejected task"
            );
            return Ok(None);
        }

        let body: TaskResponse = response.json().await?;
        if body.policy_decision == "deny" || body.granted_tasks == 0 || body.task_id.is_empty() {
            // Policy denies or clamps to zero tasks.
            tracing::info!(reasons = ?body.reasons, "policy denied or limited to zero");
            return Ok(None);
        }
        if body.blocked.unwrap_or(false) {
            self.set_blocked(true, body.blocked_reason).await;
            return Ok(None);
        }

        Ok(Some(Task {
            id: body.task_id,
            payload: body.payload.unwrap_or_default(),
            project_id: body.project_id,
            task_type: None,
        }))
    }

    async fn request_tasks_batch(&self) -> Result<Option<Vec<Task>>, reqwest::Error> {
        let url = format!("{}/v1/tasks/request_batch", self.config.scheduler_url);
        let min_batch = self.config.batch_min.max(1);
        let max_batch = self.config.batch_max.max(min_batch);
        let requested_tasks = if min_batch == max_batch {
            min_batch
        } else {
            rand::thread_rng().gen_range(min_batch..=max_batch)
        };
        let response = self
            .client
            .post(url)
            .json(&TaskBatchRequest {
                agent_uid: &self.config.node_id,
                requested_tasks,
                proposal_source: "system",
                project_id: self.config.project_id,
                allowed_task_types: self.config.allowed_task_types.clone(),
            })
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!(
                status = response.status().as_u16(),
                "scheduler rejected batch request"
            );
            return Ok(None);
        }

        let body: TaskBatchResponse = response.json().await?;
        if body.blocked.unwrap_or(false) {
            self.set_blocked(true, body.blocked_reason).await;
            return Ok(None);
        }
        if body.policy_decision == "deny" || body.granted_tasks == 0 || body.tasks.is_empty() {
            tracing::info!(reasons = ?body.reasons, "batch denied or empty");
            return Ok(None);
        }

        let tasks = body
            .tasks
            .into_iter()
            .map(|task| Task {
                id: task.task_id,
                payload: task.payload,
                project_id: Some(task.project_id),
                task_type: task.task_type,
            })
            .collect();
        Ok(Some(tasks))
    }

    async fn submit_result(&self, task: &Task, result: &str) -> Result<(), reqwest::Error> {
        let url = format!("{}/v1/tasks/submit", self.config.scheduler_url);
        let response = self
            .client
            .post(url)
            .json(&TaskSubmitRequest {
                task_id: &task.id,
                result,
                project_id: task.project_id,
                device_id: None,
            })
            .send()
            .await?;

        let body: TaskSubmitResponse = response.json().await?;
        tracing::info!(
            task_id = %task.id,
            status = %body.status,
            "submitted result"
        );
        self.log(LogLevel::Success, "Result submitted");
        Ok(())
    }
}

fn spawn_agent(agent: Agent, stop: StopSignal) -> Vec<tokio::task::JoinHandle<()>> {
    let heartbeat_agent = agent.clone();
    let heartbeat_stop = stop.clone();
    let heartbeat_handle = tokio::spawn(async move {
        heartbeat_agent.heartbeat_loop(heartbeat_stop).await;
    });

    let metrics_agent = agent.clone();
    let metrics_stop = stop.clone();
    let metrics_handle = tokio::spawn(async move {
        metrics_agent.metrics_loop(metrics_stop).await;
    });

    let run_stop = stop.clone();
    let run_handle = tokio::spawn(async move {
        agent.run_loop(run_stop).await;
    });

    vec![heartbeat_handle, metrics_handle, run_handle]
}

#[allow(dead_code)]
trait TaskRunner {
    fn name(&self) -> &'static str;
    fn run<'a>(
        &'a self,
        task: &'a Task,
        sandbox: &'a SandboxConfig,
        limits: &'a ResourceLimits,
        node_id: &'a str,
        sleep_duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>>;
}

struct SandboxRunner;

impl TaskRunner for SandboxRunner {
    fn name(&self) -> &'static str {
        "sandbox"
    }

    fn run<'a>(
        &'a self,
        task: &'a Task,
        sandbox: &'a SandboxConfig,
        limits: &'a ResourceLimits,
        node_id: &'a str,
        sleep_duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            // Dispatch by task type to keep runner extensible.
            let kind = task.payload.kind.as_deref().unwrap_or("sleep");
            match kind {
                "python_script" => run_python_task(task, sandbox, limits, node_id).await,
                _ => run_sleep_task(task, sleep_duration, node_id).await,
            }
        })
    }
}

async fn run_sleep_task(task: &Task, sleep_duration: Duration, node_id: &str) -> String {
    // Simple fallback task for MVP demos.
    tracing::info!(task_id = %task.id, runner = "sleep", "running task");
    let started_at = SystemTime::now();
    sleep(sleep_duration).await;
    let ended_at = SystemTime::now();
    let duration_ms = ended_at
        .duration_since(started_at)
        .unwrap_or_default()
        .as_millis() as u64;
    let result = SandboxResult {
        status: "ok".to_string(),
        stdout: "ok".to_string(),
        stderr: "".to_string(),
        duration_ms,
        started_at_ms: started_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        ended_at_ms: ended_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        exit_code: Some(0),
        error: None,
        stdout_bytes: 2,
        stderr_bytes: 0,
        stdout_sha256: hash_bytes("ok".as_bytes()),
        script_sha256: None,
        workspace_bytes: 0,
        files_written: 0,
        engine: "sleep".to_string(),
        node_id: node_id.to_string(),
        task_id: task.id.clone(),
    };
    serde_json::to_string(&result).unwrap_or_else(|_| "ok".to_string())
}

async fn run_python_task(
    task: &Task,
    sandbox: &SandboxConfig,
    limits: &ResourceLimits,
    node_id: &str,
) -> String {
    // Execute a python script inside a workspace with MVP safety limits.
    tracing::info!(
        task_id = %task.id,
        runner = "python",
        "running python task"
    );

    let script_bytes = match resolve_script_bytes(&task.payload).await {
        Ok(bytes) => bytes,
        Err(err) => return format!("error: {err}"),
    };
    let script_hash = hash_bytes(script_bytes.as_slice());

    let workspace = match create_workspace(task.id.as_str()) {
        Ok(path) => path,
        Err(err) => return format!("error: {err}"),
    };

    if let Err(err) = write_inputs(&workspace, task.payload.inputs.as_ref()) {
        return format!("error: {err}");
    }

    if let Err(err) = write_script(&workspace, script_bytes.as_slice()) {
        return format!("error: {err}");
    }

    if let Err(err) = enforce_workspace_limit(&workspace, sandbox.workspace_limit_bytes).await {
        return format!("error: {err}");
    }

    let (result, workspace_bytes, files_written) =
        match execute_python(&workspace, sandbox, limits, task.payload.args.as_ref()).await {
        Ok(output) => (output, dir_size(&workspace), count_files(&workspace)),
        Err(err) => {
            let now = SystemTime::now();
            let fallback = SandboxResult {
                status: "error".to_string(),
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration_ms: 0,
                started_at_ms: now
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                ended_at_ms: now
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                exit_code: None,
                error: Some(err),
                stdout_bytes: 0,
                stderr_bytes: 0,
                stdout_sha256: hash_bytes(b""),
                script_sha256: Some(script_hash),
                workspace_bytes: 0,
                files_written: 0,
                engine: "python".to_string(),
                node_id: node_id.to_string(),
                task_id: task.id.clone(),
            };
            return serde_json::to_string(&fallback).unwrap_or_else(|_| "error".to_string());
        }
    };

    let stdout_bytes = result.stdout.len() as u64;
    let stderr_bytes = result.stderr.len() as u64;
    let stdout_text = result.stdout.clone();
    let payload = SandboxResult {
        status: result.status,
        stdout: result.stdout,
        stderr: result.stderr,
        duration_ms: result.duration_ms,
        started_at_ms: result.started_at_ms,
        ended_at_ms: result.ended_at_ms,
        exit_code: result.exit_code,
        error: result.error,
        stdout_bytes,
        stderr_bytes,
        stdout_sha256: hash_bytes(stdout_text.as_bytes()),
        script_sha256: Some(script_hash),
        workspace_bytes,
        files_written,
        engine: "python".to_string(),
        node_id: node_id.to_string(),
        task_id: task.id.clone(),
    };

    serde_json::to_string(&payload).unwrap_or_else(|_| "error".to_string())
}

async fn resolve_script_bytes(payload: &TaskPayload) -> Result<Vec<u8>, String> {
    if let Some(script) = payload.script.as_deref() {
        return Ok(script.as_bytes().to_vec());
    }
    if let Some(url) = payload.script_url.as_deref() {
        let bytes = download_script(url).await?;
        if let Some(expected) = payload.script_sha256.as_deref() {
            let actual = hash_bytes(bytes.as_slice());
            if actual != expected {
                return Err("script hash mismatch".to_string());
            }
        }
        return Ok(bytes);
    }
    Err("missing script".to_string())
}

async fn download_script(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::get(url)
        .await
        .map_err(|err| format!("download script: {err}"))?;
    if !response.status().is_success() {
        return Err(format!("download script failed: {}", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("read script bytes: {err}"))?;
    Ok(bytes.to_vec())
}

fn create_workspace(task_id: &str) -> Result<PathBuf, String> {
    // Workspace lives under OS temp with a deterministic prefix.
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock error".to_string())?
        .as_millis();
    let dir_name = format!("newral_task_{}_{}", task_id, timestamp);
    let workspace = env::temp_dir().join(dir_name);
    std::fs::create_dir_all(&workspace).map_err(|err| format!("create workspace: {err}"))?;
    Ok(workspace)
}

fn write_inputs(workspace: &Path, inputs: Option<&HashMap<String, String>>) -> Result<(), String> {
    // Inputs are stored only inside the workspace.
    if let Some(inputs) = inputs {
        for (name, content) in inputs {
            if !is_safe_filename(name) {
                return Err("invalid input filename".to_string());
            }
            let path = workspace.join(name);
            std::fs::write(path, content).map_err(|err| format!("write input: {err}"))?;
        }
    }
    Ok(())
}

fn write_script(workspace: &Path, script: &[u8]) -> Result<(), String> {
    // Script is always saved as task.py inside the workspace.
    let path = workspace.join("task.py");
    std::fs::write(path, script).map_err(|err| format!("write script: {err}"))?;
    Ok(())
}

struct ExecutionOutput {
    status: String,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    started_at_ms: u128,
    ended_at_ms: u128,
    exit_code: Option<i32>,
    error: Option<String>,
}

async fn execute_python(
    workspace: &Path,
    sandbox: &SandboxConfig,
    limits: &ResourceLimits,
    args: Option<&Vec<String>>,
) -> Result<ExecutionOutput, String> {
    let python_bin = sandbox.python_bin.as_str();
    let script_path = workspace.join("task.py");

    let mut command = build_command(python_bin, script_path.as_path(), args);
    command.current_dir(workspace);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    // Spawn process with lowered priority when possible.
    let started_at = SystemTime::now();
    let mut child = command.spawn().map_err(|err| format!("spawn: {err}"))?;
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("stderr unavailable")?;

    let child = Arc::new(Mutex::new(child));
    let child_for_monitor = child.clone();

    let workspace_path = workspace.to_path_buf();
    let workspace_limit = sandbox.workspace_limit_bytes;
    let size_monitor = tokio::spawn(async move {
        // Watch workspace size and kill on breach.
        if let Err(err) =
            watch_workspace_limit(&workspace_path, workspace_limit, child_for_monitor).await
        {
            tracing::warn!(error = %err, "workspace limit reached");
        }
    });

    let stdout_handle = tokio::spawn(read_limited(
        stdout,
        sandbox.stdout_limit_bytes,
        child.clone(),
        "stdout",
    ));
    let stderr_handle = tokio::spawn(read_limited(
        stderr,
        sandbox.stderr_limit_bytes,
        child.clone(),
        "stderr",
    ));

    let child_wait = async {
        let mut guard = child.lock().await;
        guard.wait().await.map_err(|err| format!("wait: {err}"))
    };

    let status = tokio::select! {
        result = child_wait => {
            result?
        }
        _ = sleep(sandbox.timeout) => {
            // Timeout reached: terminate the process.
            let _ = child.lock().await.kill().await;
            stdout_handle.abort();
            stderr_handle.abort();
            let ended_at = SystemTime::now();
            return Ok(ExecutionOutput {
                status: "timeout".to_string(),
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration_ms: ended_at
                    .duration_since(started_at)
                    .unwrap_or_default()
                    .as_millis() as u64,
                started_at_ms: started_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                ended_at_ms: ended_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                exit_code: None,
                error: Some("timeout".to_string()),
            });
        }
        reason = monitor_resource_limits(limits.clone()), if limits.any_set() => {
            let _ = child.lock().await.kill().await;
            stdout_handle.abort();
            stderr_handle.abort();
            let ended_at = SystemTime::now();
            return Ok(ExecutionOutput {
                status: "throttled".to_string(),
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration_ms: ended_at
                    .duration_since(started_at)
                    .unwrap_or_default()
                    .as_millis() as u64,
                started_at_ms: started_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                ended_at_ms: ended_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                exit_code: None,
                error: Some(reason),
            });
        }
    };

    size_monitor.abort();

    let stdout_bytes = stdout_handle
        .await
        .map_err(|_| "stdout join error".to_string())??;
    let stderr_bytes = stderr_handle
        .await
        .map_err(|_| "stderr join error".to_string())??;
    let stdout_text = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).to_string();
    let ended_at = SystemTime::now();
    let duration_ms = ended_at
        .duration_since(started_at)
        .unwrap_or_default()
        .as_millis() as u64;

    if !status.success() {
        return Ok(ExecutionOutput {
            status: "error".to_string(),
            stdout: stdout_text.trim().to_string(),
            stderr: stderr_text.clone(),
            duration_ms,
            started_at_ms: started_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            ended_at_ms: ended_at
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            exit_code: status.code(),
            error: Some(format!("exit: {status}, stderr: {stderr_text}")),
        });
    }

    if !stderr_text.is_empty() {
        tracing::info!(stderr = %stderr_text, "python stderr");
    }

    Ok(ExecutionOutput {
        status: "ok".to_string(),
        stdout: stdout_text.trim().to_string(),
        stderr: stderr_text.trim().to_string(),
        duration_ms,
        started_at_ms: started_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        ended_at_ms: ended_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        exit_code: status.code(),
        error: None,
    })
}

async fn monitor_resource_limits(limits: ResourceLimits) -> String {
    let mut system = System::new_all();
    let mut over_count = 0;
    let mut warned_gpu = false;
    loop {
        let metrics = collect_metrics(&mut system);
        if limits.gpu_percent.is_some() && metrics.gpu_load.is_none() && !warned_gpu {
            tracing::warn!("GPU limit configured but GPU metrics unavailable");
            warned_gpu = true;
        }
        let exceeded = exceeded_limits(&metrics, &limits);
        if !exceeded.is_empty() {
            over_count += 1;
            if over_count >= 3 {
                return format!("resource limits exceeded: {}", exceeded.join(", "));
            }
        } else {
            over_count = 0;
        }
        sleep(Duration::from_secs(2)).await;
    }
}

fn build_command(python_bin: &str, script_path: &Path, args: Option<&Vec<String>>) -> Command {
    #[cfg(unix)]
    {
        // Lower priority with nice when available.
        let mut command = Command::new("nice");
        command.arg("-n").arg("10").arg(python_bin).arg(script_path);
        if let Some(args) = args {
            command.args(args);
        }
        command
    }

    #[cfg(not(unix))]
    {
        // Windows: priority lowering is a future enhancement.
        let mut command = Command::new(python_bin);
        command.arg(script_path);
        if let Some(args) = args {
            command.args(args);
        }
        #[cfg(windows)]
        {
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            // Hide console window for sandbox processes.
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }
}

async fn read_limited<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    limit_bytes: u64,
    child: Arc<Mutex<tokio::process::Child>>,
    label: &str,
) -> Result<Vec<u8>, String> {
    // Limit stdout/stderr size to protect the agent.
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|err| format!("{label} read: {err}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() as u64 > limit_bytes {
            let _ = child.lock().await.kill().await;
            return Err(format!("{label} limit exceeded"));
        }
    }

    Ok(buffer)
}

async fn watch_workspace_limit(
    workspace: &Path,
    limit_bytes: u64,
    child: Arc<Mutex<tokio::process::Child>>,
) -> Result<(), String> {
    loop {
        sleep(Duration::from_secs(1)).await;
        let size = tokio::task::spawn_blocking({
            let workspace = workspace.to_path_buf();
            move || dir_size(&workspace)
        })
        .await
        .map_err(|_| "workspace size check failed".to_string())?;

        if size > limit_bytes {
            let _ = child.lock().await.kill().await;
            return Err("workspace size limit exceeded".to_string());
        }
    }
}

async fn enforce_workspace_limit(workspace: &Path, limit_bytes: u64) -> Result<(), String> {
    let size = tokio::task::spawn_blocking({
        let workspace = workspace.to_path_buf();
        move || dir_size(&workspace)
    })
    .await
    .map_err(|_| "workspace size check failed".to_string())?;

    if size > limit_bytes {
        return Err("workspace size limit exceeded".to_string());
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    // Recursive directory size check.
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    size += dir_size(&entry.path());
                } else {
                    size += metadata.len();
                }
            }
        }
    }
    size
}

fn count_files(path: &Path) -> u64 {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    count += count_files(&entry.path());
                } else {
                    count += 1;
                }
            }
        }
    }
    count
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn is_safe_filename(name: &str) -> bool {
    // Reject paths or traversal attempts.
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn resolve_config_path() -> PathBuf {
    if let Ok(path) = env::var("AGENT_CONFIG_PATH") {
        return PathBuf::from(path);
    }

    let repo_path = PathBuf::from("client/agent/config.toml");
    if repo_path.exists() {
        return repo_path;
    }

    // Fall back to a local config next to the executable.
    PathBuf::from("config.toml")
}

fn load_config() -> Result<AgentConfig, String> {
    let config_path = resolve_config_path();

    let file_config = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).map_err(|err| format!("read config: {err}"))?;
        toml::from_str::<FileConfig>(&content).map_err(|err| format!("parse config: {err}"))?
    } else {
        FileConfig::default()
    };

    let node_id = env::var("NODE_ID")
        .ok()
        .or(file_config.node_id.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let node_id = if Uuid::parse_str(node_id.as_str()).is_ok() {
        node_id
    } else {
        Uuid::new_v4().to_string()
    };
    let display_name = env::var("NODE_DISPLAY_NAME")
        .ok()
        .or(file_config.display_name.clone());
    let scheduler_url = env::var("SCHEDULER_URL")
        .ok()
        .or(file_config.scheduler_url.clone())
        .unwrap_or_else(|| "http://localhost:8082".to_string());
    let heartbeat_interval = env::var("HEARTBEAT_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.heartbeat_interval_secs)
        .unwrap_or(10);
    let poll_interval = env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.poll_interval_secs)
        .unwrap_or(5);
    let runner_sleep = env::var("RUNNER_SLEEP_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.runner_sleep_secs)
        .unwrap_or(2);
    let batch_min = env::var("BATCH_MIN")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .or(file_config.batch_min)
        .unwrap_or(3);
    let batch_max = env::var("BATCH_MAX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .or(file_config.batch_max)
        .unwrap_or(8);
    let batch_delay_min = env::var("BATCH_DELAY_MIN_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.batch_delay_min_secs)
        .unwrap_or(15);
    let batch_delay_max = env::var("BATCH_DELAY_MAX_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.batch_delay_max_secs)
        .unwrap_or(180);
    let metrics_interval = env::var("METRICS_INTERVAL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.metrics_interval_secs)
        .unwrap_or(60);
    let python_bin = env::var("PYTHON_BIN")
        .ok()
        .or(file_config.python_bin.clone())
        .unwrap_or_else(|| "python3".to_string());
    let timeout_secs = env::var("SANDBOX_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.timeout_secs)
        .unwrap_or(60);
    let workspace_limit_mb = env::var("SANDBOX_WORKSPACE_LIMIT_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.workspace_limit_mb)
        .unwrap_or(512);
    let stdout_limit_mb = env::var("SANDBOX_STDOUT_LIMIT_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.stdout_limit_mb)
        .unwrap_or(10);
    let stderr_limit_mb = env::var("SANDBOX_STDERR_LIMIT_MB")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_config.stderr_limit_mb)
        .unwrap_or(10);
    let eula_accepted = env::var("EULA_ACCEPTED")
        .ok()
        .map(|value| value == "1")
        .or(file_config.eula_accepted)
        .unwrap_or(false);
    let project_id = env::var("PROJECT_ID")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .or(file_config.project_id);
    let allowed_task_types = env::var("ALLOWED_TASK_TYPES")
        .ok()
        .map(|value| value.split(',').map(|item| item.trim().to_string()).collect())
        .or(file_config.allowed_task_types.clone())
        .unwrap_or_default();
    let limits = ResourceLimits {
        cpu_percent: env::var("CPU_LIMIT_PERCENT")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .or(file_config.cpu_limit_percent),
        gpu_percent: env::var("GPU_LIMIT_PERCENT")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .or(file_config.gpu_limit_percent),
        ram_percent: env::var("RAM_LIMIT_PERCENT")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .or(file_config.ram_limit_percent),
    };

    let agent_config = AgentConfig {
        node_id,
        display_name,
        scheduler_url,
        heartbeat_interval: Duration::from_secs(heartbeat_interval),
        poll_interval: Duration::from_secs(poll_interval),
        runner_sleep: Duration::from_secs(runner_sleep),
        batch_min,
        batch_max,
        batch_delay_min: Duration::from_secs(batch_delay_min),
        batch_delay_max: Duration::from_secs(batch_delay_max),
        metrics_interval: Duration::from_secs(metrics_interval),
        sandbox: SandboxConfig {
            python_bin,
            timeout: Duration::from_secs(timeout_secs),
            workspace_limit_bytes: workspace_limit_mb * 1024 * 1024,
            stdout_limit_bytes: stdout_limit_mb * 1024 * 1024,
            stderr_limit_bytes: stderr_limit_mb * 1024 * 1024,
        },
        eula_accepted,
        project_id,
        allowed_task_types,
        limits,
    };

    if file_config.node_id.is_none() {
        let mut persisted = file_config.clone();
        persisted.node_id = Some(agent_config.node_id.clone());
        if persisted.eula_accepted.is_none() {
            persisted.eula_accepted = Some(agent_config.eula_accepted);
        }
        let _ = save_config(&config_path, &persisted);
    }

    Ok(agent_config)
}

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
fn save_config(path: &Path, config: &FileConfig) -> Result<(), String> {
    let content = toml::to_string_pretty(config).map_err(|err| format!("encode config: {err}"))?;
    std::fs::write(path, content).map_err(|err| format!("write config: {err}"))?;
    Ok(())
}

fn collect_hardware_info() -> serde_json::Value {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu = system.cpus().first().map(|cpu| cpu.brand().to_string());
    let cpu_freq_mhz = system.cpus().first().map(|cpu| cpu.frequency() as u64);
    let cpu_cores = system.cpus().len() as u64;
    let total_memory_mb = system.total_memory() as f64 / 1024.0;
    let os_name = System::name().unwrap_or_else(|| "unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "unknown".to_string());
    let gpu_info = collect_gpu_static_info();

    let mut disk_total_mb = 0f64;
    let mut disk_available_mb = 0f64;
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.iter() {
        disk_total_mb += disk.total_space() as f64 / 1024.0 / 1024.0;
        disk_available_mb += disk.available_space() as f64 / 1024.0 / 1024.0;
    }

    serde_json::json!({
        "cpu_model": cpu,
        "cpu_freq_mhz": cpu_freq_mhz,
        "cpu_cores": cpu_cores,
        "ram_total_mb": total_memory_mb,
        "disk_total_mb": disk_total_mb,
        "disk_available_mb": disk_available_mb,
        "gpu_model": gpu_info.as_ref().map(|(name, _)| name),
        "gpu_vram_mb": gpu_info.as_ref().map(|(_, vram)| vram),
        "os_name": os_name,
        "os_version": os_version
    })
}

fn collect_gpu_static_info() -> Option<(String, f64)> {
    let mut command = std::process::Command::new("nvidia-smi");
    command.args([
        "--query-gpu=name,memory.total",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    let mut parts = line.split(',').map(|part| part.trim());
    let name = parts.next()?.to_string();
    let vram_mb = parts.next()?.parse::<f64>().ok()?;
    Some((name, vram_mb))
}

fn collect_gpu_metrics() -> Option<(f32, f32)> {
    let mut command = std::process::Command::new("nvidia-smi");
    command.args([
        "--query-gpu=utilization.gpu,memory.used",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    let mut parts = line.split(',').map(|part| part.trim());
    let util = parts.next()?.parse::<f32>().ok()?;
    let mem_used = parts.next()?.parse::<f32>().ok()?;
    Some((util, mem_used))
}

fn collect_metrics(system: &mut System) -> AgentMetrics {
    system.refresh_cpu();
    system.refresh_memory();
    let mut networks = Networks::new_with_refreshed_list();
    networks.refresh();

    let cpu_load = Some(system.global_cpu_info().cpu_usage());
    let ram_used_mb = Some(system.used_memory() as f32 / 1024.0);
    let ram_total_mb = Some(system.total_memory() as f32 / 1024.0);
    let (gpu_load, gpu_mem_used_mb) = collect_gpu_metrics()
        .map(|(load, used)| (Some(load), Some(used)))
        .unwrap_or((None, None));

    let mut net_rx: i64 = 0;
    let mut net_tx: i64 = 0;
    for (_name, data) in networks.iter() {
        net_rx += data.received() as i64;
        net_tx += data.transmitted() as i64;
    }

    AgentMetrics {
        cpu_load,
        ram_used_mb,
        ram_total_mb,
        gpu_load,
        gpu_mem_used_mb,
        net_rx_bytes: Some(net_rx),
        net_tx_bytes: Some(net_tx),
        disk_read_bytes: None,
        disk_write_bytes: None,
    }
}

fn exceeded_limits(metrics: &AgentMetrics, limits: &ResourceLimits) -> Vec<String> {
    let mut exceeded = Vec::new();
    if let (Some(limit), Some(current)) = (limits.cpu_percent, metrics.cpu_load) {
        if current > limit {
            exceeded.push("cpu".to_string());
        }
    }
    if let (Some(limit), Some(used), Some(total)) =
        (limits.ram_percent, metrics.ram_used_mb, metrics.ram_total_mb)
    {
        let percent = (used / total) * 100.0;
        if percent > limit {
            exceeded.push("ram".to_string());
        }
    }
    if let (Some(limit), Some(current)) = (limits.gpu_percent, metrics.gpu_load) {
        if current > limit {
            exceeded.push("gpu".to_string());
        }
    }
    exceeded
}

#[cfg(feature = "gui")]
mod gui {
    use super::*;
    use eframe::egui;
    use serde_json::Value;

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum AgentSection {
        Overview,
        Settings,
        Limits,
        Logs,
    }

    pub fn run() {
        let log_buffer = LogBuffer::new(500);
        init_tracing(Some(log_buffer.clone()));

        let options = eframe::NativeOptions::default();
        let app = AgentGui::new(log_buffer);
        let _ = eframe::run_native("Newral Agent", options, Box::new(|_cc| Ok(Box::new(app))));
    }

    struct AgentGui {
        config_path: PathBuf,
        log_buffer: LogBuffer,
        status_state: Arc<Mutex<AgentRuntimeState>>,
        base_config: AgentConfig,
        node_id: String,
        display_name: String,
        protocol: String,
        host: String,
        running: bool,
        stop: Option<StopSignal>,
        runtime: tokio::runtime::Runtime,
        last_error: Option<String>,
        project_id: String,
        allowed_task_types: String,
        cpu_limit: String,
        gpu_limit: String,
        ram_limit: String,
        eula_required: bool,
        eula_scrolled: bool,
        show_info: bool,
        show_warn: bool,
        show_error: bool,
        show_success: bool,
        section: AgentSection,
        hardware_snapshot: Value,
        last_metrics: Option<AgentMetrics>,
        last_metrics_at: Instant,
    }

    impl AgentGui {
        fn new(log_buffer: LogBuffer) -> Self {
            let config_path = resolve_config_path();
            let base_config = load_config().unwrap_or_else(|_| AgentConfig {
                node_id: "dev-node".to_string(),
                display_name: None,
                scheduler_url: "http://localhost:8082".to_string(),
                heartbeat_interval: Duration::from_secs(10),
                poll_interval: Duration::from_secs(5),
                runner_sleep: Duration::from_secs(2),
                batch_min: 3,
                batch_max: 8,
                batch_delay_min: Duration::from_secs(15),
                batch_delay_max: Duration::from_secs(180),
                metrics_interval: Duration::from_secs(60),
                sandbox: SandboxConfig {
                    python_bin: "python".to_string(),
                    timeout: Duration::from_secs(60),
                    workspace_limit_bytes: 512 * 1024 * 1024,
                    stdout_limit_bytes: 10 * 1024 * 1024,
                    stderr_limit_bytes: 10 * 1024 * 1024,
                },
                eula_accepted: false,
                project_id: None,
                allowed_task_types: Vec::new(),
                limits: ResourceLimits::default(),
            });

            let (protocol, host) = split_url(&base_config.scheduler_url);
            let status_state = Arc::new(Mutex::new(AgentRuntimeState::default()));

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");

            Self {
                config_path,
                log_buffer,
                status_state,
                base_config: base_config.clone(),
                node_id: base_config.node_id,
                display_name: base_config.display_name.unwrap_or_default(),
                protocol,
                host,
                running: false,
                stop: None,
                runtime,
                last_error: None,
                project_id: base_config
                    .project_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                allowed_task_types: base_config.allowed_task_types.join(", "),
                cpu_limit: base_config
                    .limits
                    .cpu_percent
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                gpu_limit: base_config
                    .limits
                    .gpu_percent
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                ram_limit: base_config
                    .limits
                    .ram_percent
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                eula_required: !base_config.eula_accepted,
                eula_scrolled: false,
                show_info: true,
                show_warn: true,
                show_error: true,
                show_success: true,
                section: AgentSection::Overview,
                hardware_snapshot: collect_hardware_info(),
                last_metrics: None,
                last_metrics_at: Instant::now(),
            }
        }

        fn scheduler_url(&self) -> String {
            format!("{}://{}", self.protocol, self.host)
        }

        fn start_agent(&mut self) {
            if self.running {
                return;
            }
            let mut config = self.base_config.clone();
            config.node_id = self.node_id.trim().to_string();
            config.display_name = if self.display_name.trim().is_empty() {
                None
            } else {
                Some(self.display_name.trim().to_string())
            };
            config.scheduler_url = self.scheduler_url();
            config.project_id = self.project_id.trim().parse::<i64>().ok();
            config.allowed_task_types = self
                .allowed_task_types
                .split(',')
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            config.limits = ResourceLimits {
                cpu_percent: self.cpu_limit.trim().parse::<f32>().ok(),
                gpu_percent: self.gpu_limit.trim().parse::<f32>().ok(),
                ram_percent: self.ram_limit.trim().parse::<f32>().ok(),
            };
            config.eula_accepted = !self.eula_required;

            let stop = StopSignal::new();
            let runtime_state = self.status_state.clone();
            let runtime = AgentRuntime::with_state(self.log_buffer.clone(), runtime_state);
            let agent = Agent {
                config: std::sync::Arc::new(config),
                client: Client::new(),
                runner: std::sync::Arc::new(SandboxRunner {}),
                runtime: Some(runtime),
            };

            let stop_clone = stop.clone();
            let state_clone = self.status_state.clone();
            let log_clone = self.log_buffer.clone();
            self.runtime.spawn(async move {
                match agent.register().await {
                    Ok(response) => {
                        if response.blocked {
                            let mut status = state_clone.lock().await;
                            status.blocked = true;
                            status.blocked_reason = response.blocked_reason;
                            log_clone.push_line(LogLevel::Error, "Agent blocked by server");
                            return;
                        }
                    }
                    Err(err) => {
                        let mut status = state_clone.lock().await;
                        status.last_error = Some(format!("registration: {err}"));
                        log_clone.push_line(LogLevel::Error, "Agent registration failed");
                        return;
                    }
                }
                let _handles = spawn_agent(agent, stop_clone);
            });

            self.running = true;
            self.stop = Some(stop);
            self.last_error = None;
            self.log_buffer
                .push_line(LogLevel::Success, "Agent started");
            tracing::info!("agent started");
        }

        fn stop_agent(&mut self) {
            if let Some(stop) = self.stop.take() {
                stop.stop();
            }
            self.running = false;
            {
                let mut status = self.runtime.block_on(self.status_state.lock());
                status.connected = false;
                status.current_task = None;
            }
            self.log_buffer.push_line(LogLevel::Warn, "Agent stopped");
            tracing::info!("agent stopped");
        }

        fn save_settings(&mut self) {
            let project_id = self.project_id.trim().parse::<i64>().ok();
            let allowed_task_types = self
                .allowed_task_types
                .split(',')
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            let cpu_limit = self.cpu_limit.trim().parse::<f32>().ok();
            let gpu_limit = self.gpu_limit.trim().parse::<f32>().ok();
            let ram_limit = self.ram_limit.trim().parse::<f32>().ok();
            let file_config = FileConfig {
                node_id: Some(self.node_id.trim().to_string()),
                display_name: Some(self.display_name.trim().to_string()),
                scheduler_url: Some(self.scheduler_url()),
                heartbeat_interval_secs: Some(self.base_config.heartbeat_interval.as_secs()),
                poll_interval_secs: Some(self.base_config.poll_interval.as_secs()),
                runner_sleep_secs: Some(self.base_config.runner_sleep.as_secs()),
                batch_min: Some(self.base_config.batch_min),
                batch_max: Some(self.base_config.batch_max),
                batch_delay_min_secs: Some(self.base_config.batch_delay_min.as_secs()),
                batch_delay_max_secs: Some(self.base_config.batch_delay_max.as_secs()),
                metrics_interval_secs: Some(self.base_config.metrics_interval.as_secs()),
                python_bin: Some(self.base_config.sandbox.python_bin.clone()),
                timeout_secs: Some(self.base_config.sandbox.timeout.as_secs()),
                workspace_limit_mb: Some(
                    self.base_config.sandbox.workspace_limit_bytes / 1024 / 1024,
                ),
                stdout_limit_mb: Some(self.base_config.sandbox.stdout_limit_bytes / 1024 / 1024),
                stderr_limit_mb: Some(self.base_config.sandbox.stderr_limit_bytes / 1024 / 1024),
                eula_accepted: Some(!self.eula_required),
                project_id,
                allowed_task_types: Some(allowed_task_types),
                cpu_limit_percent: cpu_limit,
                gpu_limit_percent: gpu_limit,
                ram_limit_percent: ram_limit,
            };

            if let Err(err) = save_config(&self.config_path, &file_config) {
                self.last_error = Some(err);
            } else {
                self.last_error = None;
            }
        }
    }

    impl eframe::App for AgentGui {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            apply_portal_style(ctx);
            let status_snapshot = self.runtime.block_on(self.status_state.lock()).clone();
            let connected = status_snapshot.connected;
            let paused = status_snapshot.paused;
            let blocked = status_snapshot.blocked;
            let blocked_reason = status_snapshot
                .blocked_reason
                .clone()
                .unwrap_or_else(|| "none".to_string());
            let current_task = status_snapshot
                .current_task
                .clone()
                .unwrap_or_else(|| "idle".to_string());
            let last_result = status_snapshot
                .last_result
                .clone()
                .unwrap_or_else(|| "n/a".to_string());
            let last_error = status_snapshot
                .last_error
                .clone()
                .unwrap_or_else(|| "none".to_string());
            let status_color = if connected {
                egui::Color32::from_rgb(31, 139, 76)
            } else {
                egui::Color32::from_rgb(148, 158, 170)
            };
            let ink = egui::Color32::from_rgb(15, 27, 42);
            let muted = egui::Color32::from_rgb(91, 107, 125);
            let accent = egui::Color32::from_rgb(26, 140, 255);
            let line = egui::Color32::from_rgba_premultiplied(15, 27, 42, 20);

            if self.last_metrics_at.elapsed() >= Duration::from_secs(5) {
                let mut system = System::new_all();
                self.last_metrics = Some(collect_metrics(&mut system));
                self.last_metrics_at = Instant::now();
            }

            if self.eula_required {
                egui::Window::new("End User License Agreement")
                    .collapsible(false)
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.label("Please review and accept the EULA to continue.");
                        let scroll = egui::ScrollArea::vertical()
                            .max_height(240.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(EULA_TEXT)
                                        .wrap()
                                        .selectable(true),
                                );
                            });
                        let at_bottom = scroll.state.offset.y + scroll.inner_rect.height()
                            >= scroll.content_size.y - 4.0;
                        if at_bottom {
                            self.eula_scrolled = true;
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(self.eula_scrolled, egui::Button::new("Accept"))
                                .clicked()
                            {
                                self.eula_required = false;
                                self.base_config.eula_accepted = true;
                                self.save_settings();
                            }
                            if ui.button("Decline").clicked() {
                                self.last_error = Some("EULA declined".to_string());
                            }
                        });
                    });
            }

            egui::TopBottomPanel::top("top_bar")
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(255, 255, 255))
                        .stroke(egui::Stroke::new(1.0, line))
                        .inner_margin(egui::Margin::symmetric(16.0, 12.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("NEWRAL")
                                .strong()
                                .size(18.0)
                                .color(accent),
                        );
                        ui.label(
                            egui::RichText::new("Agent")
                                .strong()
                                .size(18.0)
                                .color(ink),
                        );
                        ui.add_space(12.0);
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(239, 244, 252))
                            .rounding(egui::Rounding::same(10.0))
                            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(status_color, "●");
                                    ui.label(if connected { "Connected" } else { "Offline" });
                                });
                            });
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!("v{}", AGENT_VERSION))
                                .size(12.0)
                                .color(muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.running {
                                let pause_label = if paused { "Resume" } else { "Pause" };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(pause_label)
                                                .color(ink)
                                                .strong(),
                                        )
                                        .fill(egui::Color32::from_rgb(235, 241, 252)),
                                    )
                                    .clicked()
                                {
                                    let mut status = self.runtime.block_on(self.status_state.lock());
                                    status.paused = !paused;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("Stop")
                                                .color(egui::Color32::from_rgb(132, 24, 24))
                                                .strong(),
                                        )
                                        .fill(egui::Color32::from_rgb(255, 232, 232)),
                                    )
                                    .clicked()
                                {
                                    self.stop_agent();
                                }
                            } else if ui
                                .add_enabled(
                                    !self.eula_required,
                                    egui::Button::new(
                                        egui::RichText::new("Start")
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(accent),
                                )
                                .clicked()
                            {
                                self.start_agent();
                            }
                        });
                    });
                });

            egui::SidePanel::left("nav_panel")
                .frame(
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(255, 255, 255))
                        .stroke(egui::Stroke::new(1.0, line))
                        .inner_margin(egui::Margin::same(16.0)),
                )
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new("Operations")
                            .strong()
                            .color(ink),
                    );
                    ui.label(
                        egui::RichText::new("Control center")
                            .size(12.0)
                            .color(muted),
                    );
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.selectable_value(&mut self.section, AgentSection::Overview, "Overview");
                    ui.selectable_value(&mut self.section, AgentSection::Settings, "Settings");
                    ui.selectable_value(&mut self.section, AgentSection::Limits, "Limits");
                    ui.selectable_value(&mut self.section, AgentSection::Logs, "Logs");
                    ui.add_space(12.0);
                    ui.separator();
                    if blocked {
                        ui.colored_label(egui::Color32::from_rgb(200, 46, 46), format!("Blocked: {}", blocked_reason));
                    } else if paused {
                        ui.colored_label(egui::Color32::from_rgb(179, 122, 10), "Paused");
                    }
                    if let Some(err) = &self.last_error {
                        ui.colored_label(egui::Color32::from_rgb(200, 46, 46), err);
                    }
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.section {
                    AgentSection::Overview => {
                        ui.label(
                            egui::RichText::new("Overview")
                                .size(20.0)
                                .strong()
                                .color(ink),
                        );
                        ui.label(
                            egui::RichText::new("Live status and capacity at a glance.")
                                .size(12.0)
                                .color(muted),
                        );
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            portal_card(ui, |ui| {
                                ui.label(egui::RichText::new("Connection").color(muted));
                                ui.colored_label(status_color, if connected { "Online" } else { "Offline" });
                                ui.label(format!(
                                    "Project ID: {}",
                                    if self.project_id.is_empty() { "auto" } else { &self.project_id }
                                ));
                            });
                            portal_card(ui, |ui| {
                                ui.label(egui::RichText::new("Current task").color(muted));
                                ui.label(current_task);
                                ui.label(format!("Last result: {}", last_result));
                            });
                            portal_card(ui, |ui| {
                                ui.label(egui::RichText::new("Agent health").color(muted));
                                ui.label(format!("Last error: {}", last_error));
                            });
                        });

                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new("Hardware profile")
                                .size(16.0)
                                .strong()
                                .color(ink),
                        );
                        ui.label(
                            egui::RichText::new("Reported on registration.")
                                .size(12.0)
                                .color(muted),
                        );
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(8.0);
                        let cpu = self
                            .hardware_snapshot
                            .get("cpu_model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let ram = self
                            .hardware_snapshot
                            .get("ram_total_mb")
                            .and_then(|v| v.as_f64())
                            .map(|v| format!("{:.0} MB", v))
                            .unwrap_or_else(|| "unknown".to_string());
                        let disk = self
                            .hardware_snapshot
                            .get("disk_total_mb")
                            .and_then(|v| v.as_f64())
                            .map(|v| format!("{:.0} MB", v))
                            .unwrap_or_else(|| "unknown".to_string());
                        let gpu = self
                            .hardware_snapshot
                            .get("gpu_model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("none");
                        portal_card(ui, |ui| {
                            ui.label(format!("CPU: {cpu}"));
                            ui.label(format!("RAM: {ram}"));
                            ui.label(format!("Disk: {disk}"));
                            ui.label(format!("GPU: {gpu}"));
                        });

                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new("Live metrics")
                                .size(16.0)
                                .strong()
                                .color(ink),
                        );
                        ui.label(
                            egui::RichText::new("Updated every few seconds.")
                                .size(12.0)
                                .color(muted),
                        );
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(8.0);
                        portal_card(ui, |ui| {
                            if let Some(metrics) = &self.last_metrics {
                                ui.label(format!(
                                    "CPU: {}%  RAM: {} / {} MB",
                                    metrics
                                        .cpu_load
                                        .map(|v| format!("{v:.1}"))
                                        .unwrap_or_else(|| "—".to_string()),
                                    metrics
                                        .ram_used_mb
                                        .map(|v| format!("{v:.0}"))
                                        .unwrap_or_else(|| "—".to_string()),
                                    metrics
                                        .ram_total_mb
                                        .map(|v| format!("{v:.0}"))
                                        .unwrap_or_else(|| "—".to_string()),
                                ));
                                ui.label(format!(
                                    "Net RX: {}  Net TX: {}",
                                    metrics
                                        .net_rx_bytes
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".to_string()),
                                    metrics
                                        .net_tx_bytes
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".to_string()),
                                ));
                            } else {
                                ui.label("Metrics collecting...");
                            }
                        });
                    }
                    AgentSection::Settings => {
                        ui.heading("Settings");
                        ui.separator();
                        ui.label("Node ID");
                        ui.text_edit_singleline(&mut self.node_id);
                        ui.label("Display name");
                        ui.text_edit_singleline(&mut self.display_name);
                        ui.label("Project ID");
                        ui.text_edit_singleline(&mut self.project_id);
                        ui.label("Allowed task types (comma separated)");
                        ui.text_edit_singleline(&mut self.allowed_task_types);
                        ui.separator();
                        ui.label("Server");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.protocol, "http".to_string(), "HTTP");
                            ui.radio_value(&mut self.protocol, "https".to_string(), "HTTPS");
                        });
                        ui.text_edit_singleline(&mut self.host);
                        ui.label(format!("URL: {}", self.scheduler_url()));
                        if ui.button("Save settings").clicked() {
                            self.save_settings();
                        }
                    }
                    AgentSection::Limits => {
                        ui.heading("Resource limits");
                        ui.separator();
                        ui.label("CPU limit (%)");
                        ui.text_edit_singleline(&mut self.cpu_limit);
                        ui.label("GPU limit (%)");
                        ui.text_edit_singleline(&mut self.gpu_limit);
                        ui.label("RAM limit (%)");
                        ui.text_edit_singleline(&mut self.ram_limit);
                        if ui.button("Save limits").clicked() {
                            self.save_settings();
                        }
                    }
                    AgentSection::Logs => {
                        ui.heading("Logs");
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.show_info, "Info");
                            ui.checkbox(&mut self.show_warn, "Warn");
                            ui.checkbox(&mut self.show_error, "Error");
                            ui.checkbox(&mut self.show_success, "Success");
                        });
                        ui.separator();
                        let lines = self.log_buffer.snapshot();
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for entry in lines {
                                    let show = match entry.level {
                                        LogLevel::Info => self.show_info,
                                        LogLevel::Warn => self.show_warn,
                                        LogLevel::Error => self.show_error,
                                        LogLevel::Success => self.show_success,
                                    };
                                    if !show {
                                        continue;
                                    }
                                    let color = match entry.level {
                                        LogLevel::Info => egui::Color32::from_rgb(34, 46, 60),
                                        LogLevel::Warn => egui::Color32::from_rgb(179, 122, 10),
                                        LogLevel::Error => egui::Color32::from_rgb(196, 48, 48),
                                        LogLevel::Success => egui::Color32::from_rgb(31, 139, 76),
                                    };
                                    ui.colored_label(
                                        color,
                                        format!("[{}] {}", entry.level.label(), entry.message),
                                    );
                                }
                            });
                    }
                }
                });

            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    fn split_url(url: &str) -> (String, String) {
        if let Some(rest) = url.strip_prefix("https://") {
            return ("https".to_string(), rest.to_string());
        }
        if let Some(rest) = url.strip_prefix("http://") {
            return ("http".to_string(), rest.to_string());
        }
        ("http".to_string(), url.to_string())
    }
}

#[cfg(feature = "gui")]
fn apply_portal_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.window_rounding = egui::Rounding::same(18.0);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(12.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(12.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(12.0);
    style.visuals.panel_fill = egui::Color32::from_rgb(246, 248, 251);
    style.visuals.window_fill = egui::Color32::from_rgb(246, 248, 251);
    style.visuals.faint_bg_color = egui::Color32::from_rgb(255, 255, 255);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(255, 255, 255);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(26, 140, 255);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(26, 140, 255));
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(255, 255, 255);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(235, 241, 252);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(26, 140, 255);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 27, 42));
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(15, 27, 42));
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(15, 27, 42));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.spacing.item_spacing = egui::vec2(12.0, 10.0);
    style.spacing.window_margin = egui::Margin::same(16.0);
    ctx.set_style(style);
}

#[cfg(feature = "gui")]
fn portal_card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(255, 255, 255))
        .rounding(egui::Rounding::same(14.0))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(15, 27, 42, 20)))
        .shadow(egui::epaint::Shadow {
            offset: egui::vec2(0.0, 12.0),
            blur: 24.0,
            spread: 0.0,
            color: egui::Color32::from_rgba_premultiplied(15, 27, 42, 18),
        })
        .inner_margin(egui::Margin::same(12.0));
    frame.show(ui, add);
}
