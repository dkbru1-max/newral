#![cfg_attr(feature = "gui", windows_subsystem = "windows")]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    env,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{Mutex, Notify},
    time::sleep,
};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Debug, Clone)]
struct AgentConfig {
    node_id: String,
    scheduler_url: String,
    heartbeat_interval: Duration,
    poll_interval: Duration,
    runner_sleep: Duration,
    sandbox: SandboxConfig,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct FileConfig {
    node_id: Option<String>,
    scheduler_url: Option<String>,
    heartbeat_interval_secs: Option<u64>,
    poll_interval_secs: Option<u64>,
    runner_sleep_secs: Option<u64>,
    python_bin: Option<String>,
    timeout_secs: Option<u64>,
    workspace_limit_mb: Option<u64>,
    stdout_limit_mb: Option<u64>,
    stderr_limit_mb: Option<u64>,
}

#[derive(Clone)]
struct Agent {
    config: std::sync::Arc<AgentConfig>,
    client: Client,
    runner: std::sync::Arc<dyn TaskRunner + Send + Sync>,
}

#[derive(Serialize)]
struct HeartbeatRequest<'a> {
    node_id: &'a str,
}

#[derive(Serialize)]
struct TaskRequest<'a> {
    node_id: &'a str,
    requested_tasks: u32,
    proposal_source: &'a str,
}

#[derive(Deserialize, Default)]
struct TaskResponse {
    status: String,
    task_id: String,
    policy_decision: String,
    granted_tasks: u32,
    reasons: Vec<String>,
    payload: Option<TaskPayload>,
    project_id: Option<i64>,
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

#[derive(Debug)]
struct Task {
    id: String,
    payload: TaskPayload,
    project_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TaskPayload {
    kind: Option<String>,
    script: Option<String>,
    inputs: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
struct SandboxConfig {
    python_bin: String,
    timeout: Duration,
    workspace_limit_bytes: u64,
    stdout_limit_bytes: u64,
    stderr_limit_bytes: u64,
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

#[derive(Clone)]
struct LogBuffer {
    lines: Arc<StdMutex<VecDeque<String>>>,
    limit: usize,
}

impl LogBuffer {
    fn new(limit: usize) -> Self {
        Self {
            lines: Arc::new(StdMutex::new(VecDeque::new())),
            limit,
        }
    }

    fn push(&self, line: String) {
        let mut lines = self.lines.lock().unwrap();
        lines.push_back(line);
        while lines.len() > self.limit {
            lines.pop_front();
        }
    }

    fn snapshot(&self) -> Vec<String> {
        self.lines.lock().unwrap().iter().cloned().collect()
    }
}

struct LogWriter {
    buffer: LogBuffer,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let line = String::from_utf8_lossy(buf).to_string();
        self.buffer.push(line);
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

fn init_tracing(log_buffer: Option<LogBuffer>) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = FmtSubscriber::builder().with_env_filter(filter).with_ansi(false);
    let writer = match log_buffer {
        Some(buffer) => tracing_subscriber::fmt::writer::BoxMakeWriter::new(buffer),
        None => tracing_subscriber::fmt::writer::BoxMakeWriter::new(io::stdout),
    };
    let subscriber = builder.with_writer(writer).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg(feature = "gui")]
fn main() {
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
    };

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

impl Agent {
    async fn heartbeat_loop(self, stop: StopSignal) {
        loop {
            if stop.stopped() {
                break;
            }
            if let Err(err) = self.send_heartbeat().await {
                tracing::warn!(error = %err, "heartbeat failed");
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

        tracing::info!(
            status = response.status().as_u16(),
            "heartbeat sent"
        );
        Ok(())
    }

    async fn run_loop(&self, stop: StopSignal) {
        loop {
            if stop.stopped() {
                break;
            }
            match self.request_task().await {
                Ok(Some(task)) => {
                    // Runner abstracts future sandboxed execution.
                    let result = self
                        .runner
                        .run(&task, &self.config.sandbox, self.config.runner_sleep)
                        .await;
                    if let Err(err) = self.submit_result(&task, &result).await {
                        tracing::warn!(error = %err, "submit failed");
                    }
                }
                Ok(None) => {
                    tracing::debug!("no task available");
                }
                Err(err) => {
                    tracing::warn!(error = %err, "task request failed");
                }
            }

            if stop.sleep_or_stop(self.config.poll_interval).await {
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
                requested_tasks: 1,
                proposal_source: "system",
            })
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!(status = response.status().as_u16(), "scheduler rejected task");
            return Ok(None);
        }

        let body: TaskResponse = response.json().await?;
        if body.policy_decision == "deny" || body.granted_tasks == 0 || body.task_id.is_empty() {
            // Policy denies or clamps to zero tasks.
            tracing::info!(reasons = ?body.reasons, "policy denied or limited to zero");
            return Ok(None);
        }

        Ok(Some(Task {
            id: body.task_id,
            payload: body.payload.unwrap_or_default(),
            project_id: body.project_id,
        }))
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
        Ok(())
    }
}

fn spawn_agent(agent: Agent, stop: StopSignal) -> Vec<tokio::task::JoinHandle<()>> {
    let heartbeat_agent = agent.clone();
    let heartbeat_stop = stop.clone();
    let heartbeat_handle = tokio::spawn(async move {
        heartbeat_agent.heartbeat_loop(heartbeat_stop).await;
    });

    let run_stop = stop.clone();
    let run_handle = tokio::spawn(async move {
        agent.run_loop(run_stop).await;
    });

    vec![heartbeat_handle, run_handle]
}

trait TaskRunner {
    fn name(&self) -> &'static str;
    fn run<'a>(
        &'a self,
        task: &'a Task,
        sandbox: &'a SandboxConfig,
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
        sleep_duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            // Dispatch by task type to keep runner extensible.
            let kind = task.payload.kind.as_deref().unwrap_or("sleep");
            match kind {
                "python_script" => run_python_task(task, sandbox).await,
                _ => run_sleep_task(task, sleep_duration).await,
            }
        })
    }
}

async fn run_sleep_task(task: &Task, sleep_duration: Duration) -> String {
    // Simple fallback task for MVP demos.
    tracing::info!(task_id = %task.id, runner = "sleep", "running task");
    sleep(sleep_duration).await;
    "ok".to_string()
}

async fn run_python_task(task: &Task, sandbox: &SandboxConfig) -> String {
    // Execute a python script inside a workspace with MVP safety limits.
    tracing::info!(
        task_id = %task.id,
        runner = "python",
        "running python task"
    );

    let Some(script) = task.payload.script.as_deref() else {
        return "error: missing script".to_string();
    };

    let workspace = match create_workspace(task.id.as_str()) {
        Ok(path) => path,
        Err(err) => return format!("error: {err}"),
    };

    if let Err(err) = write_inputs(&workspace, task.payload.inputs.as_ref()) {
        return format!("error: {err}");
    }

    if let Err(err) = write_script(&workspace, script) {
        return format!("error: {err}");
    }

    if let Err(err) = enforce_workspace_limit(&workspace, sandbox.workspace_limit_bytes).await {
        return format!("error: {err}");
    }

    match execute_python(&workspace, sandbox).await {
        Ok(output) => output,
        Err(err) => format!("error: {err}"),
    }
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

fn write_inputs(
    workspace: &Path,
    inputs: Option<&HashMap<String, String>>,
) -> Result<(), String> {
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

fn write_script(workspace: &Path, script: &str) -> Result<(), String> {
    // Script is always saved as task.py inside the workspace.
    let path = workspace.join("task.py");
    std::fs::write(path, script).map_err(|err| format!("write script: {err}"))?;
    Ok(())
}

async fn execute_python(workspace: &Path, sandbox: &SandboxConfig) -> Result<String, String> {
    let python_bin = sandbox.python_bin.as_str();
    let script_path = workspace.join("task.py");

    let mut command = build_command(python_bin, script_path.as_path());
    command.current_dir(workspace);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    // Spawn process with lowered priority when possible.
    let mut child = command.spawn().map_err(|err| format!("spawn: {err}"))?;
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("stderr unavailable")?;

    let child = Arc::new(Mutex::new(child));
    let child_for_monitor = child.clone();

    let workspace_path = workspace.to_path_buf();
    let workspace_limit = sandbox.workspace_limit_bytes;
    let size_monitor = tokio::spawn(async move {
        // Watch workspace size and kill on breach.
        if let Err(err) = watch_workspace_limit(&workspace_path, workspace_limit, child_for_monitor)
            .await
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

    let status = tokio::select! {
        result = child.lock().await.wait() => {
            result.map_err(|err| format!("wait: {err}"))?
        }
        _ = sleep(sandbox.timeout) => {
            // Timeout reached: terminate the process.
            let _ = child.lock().await.kill().await;
            stdout_handle.abort();
            stderr_handle.abort();
            return Err("timeout".to_string());
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

    // CPU throttling hook: observe usage and react in future versions.
    tracing::info!(cpu_throttle = "not_enforced", "cpu monitor placeholder");

    if !status.success() {
        return Err(format!("exit: {status}, stderr: {stderr_text}"));
    }

    if !stderr_text.is_empty() {
        tracing::info!(stderr = %stderr_text, "python stderr");
    }

    Ok(stdout_text.trim().to_string())
}

fn build_command(python_bin: &str, script_path: &Path) -> Command {
    #[cfg(unix)]
    {
        // Lower priority with nice when available.
        let mut command = Command::new("nice");
        command.arg("-n").arg("10").arg(python_bin).arg(script_path);
        command
    }

    #[cfg(not(unix))]
    {
        // Windows: priority lowering is a future enhancement.
        let mut command = Command::new(python_bin);
        command.arg(script_path);
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
        let content = std::fs::read_to_string(&config_path)
            .map_err(|err| format!("read config: {err}"))?;
        toml::from_str::<FileConfig>(&content).map_err(|err| format!("parse config: {err}"))?
    } else {
        FileConfig::default()
    };

    let node_id = env::var("NODE_ID")
        .ok()
        .or(file_config.node_id)
        .unwrap_or_else(|| "dev-node".to_string());
    let scheduler_url = env::var("SCHEDULER_URL")
        .ok()
        .or(file_config.scheduler_url)
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
    let python_bin = env::var("PYTHON_BIN")
        .ok()
        .or(file_config.python_bin)
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

    Ok(AgentConfig {
        node_id,
        scheduler_url,
        heartbeat_interval: Duration::from_secs(heartbeat_interval),
        poll_interval: Duration::from_secs(poll_interval),
        runner_sleep: Duration::from_secs(runner_sleep),
        sandbox: SandboxConfig {
            python_bin,
            timeout: Duration::from_secs(timeout_secs),
            workspace_limit_bytes: workspace_limit_mb * 1024 * 1024,
            stdout_limit_bytes: stdout_limit_mb * 1024 * 1024,
            stderr_limit_bytes: stderr_limit_mb * 1024 * 1024,
        },
    })
}

fn save_config(path: &Path, config: &FileConfig) -> Result<(), String> {
    let content =
        toml::to_string_pretty(config).map_err(|err| format!("encode config: {err}"))?;
    std::fs::write(path, content).map_err(|err| format!("write config: {err}"))?;
    Ok(())
}

#[cfg(feature = "gui")]
mod gui {
    use super::*;
    use eframe::egui;

    pub fn run() {
        let log_buffer = LogBuffer::new(500);
        init_tracing(Some(log_buffer.clone()));

        let options = eframe::NativeOptions::default();
        let app = AgentGui::new(log_buffer);
        let _ = eframe::run_native(
            "Newral Agent",
            options,
            Box::new(|_cc| Ok(Box::new(app))),
        );
    }

    struct AgentGui {
        config_path: PathBuf,
        log_buffer: LogBuffer,
        base_config: AgentConfig,
        node_id: String,
        protocol: String,
        host: String,
        running: bool,
        stop: Option<StopSignal>,
        runtime: tokio::runtime::Runtime,
        last_error: Option<String>,
    }

    impl AgentGui {
        fn new(log_buffer: LogBuffer) -> Self {
            let config_path = resolve_config_path();
            let base_config = load_config().unwrap_or_else(|_| AgentConfig {
                node_id: "dev-node".to_string(),
                scheduler_url: "http://localhost:8082".to_string(),
                heartbeat_interval: Duration::from_secs(10),
                poll_interval: Duration::from_secs(5),
                runner_sleep: Duration::from_secs(2),
                sandbox: SandboxConfig {
                    python_bin: "python".to_string(),
                    timeout: Duration::from_secs(60),
                    workspace_limit_bytes: 512 * 1024 * 1024,
                    stdout_limit_bytes: 10 * 1024 * 1024,
                    stderr_limit_bytes: 10 * 1024 * 1024,
                },
            });

            let (protocol, host) = split_url(&base_config.scheduler_url);

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");

            Self {
                config_path,
                log_buffer,
                base_config: base_config.clone(),
                node_id: base_config.node_id,
                protocol,
                host,
                running: false,
                stop: None,
                runtime,
                last_error: None,
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
            config.scheduler_url = self.scheduler_url();

            let stop = StopSignal::new();
            let agent = Agent {
                config: std::sync::Arc::new(config),
                client: Client::new(),
                runner: std::sync::Arc::new(SandboxRunner {}),
            };

            let stop_clone = stop.clone();
            self.runtime.spawn(async move {
                let _handles = spawn_agent(agent, stop_clone);
            });

            self.running = true;
            self.stop = Some(stop);
            self.last_error = None;
            tracing::info!("agent started");
        }

        fn stop_agent(&mut self) {
            if let Some(stop) = self.stop.take() {
                stop.stop();
            }
            self.running = false;
            tracing::info!("agent stopped");
        }

        fn save_settings(&mut self) {
            let file_config = FileConfig {
                node_id: Some(self.node_id.trim().to_string()),
                scheduler_url: Some(self.scheduler_url()),
                heartbeat_interval_secs: Some(self.base_config.heartbeat_interval.as_secs()),
                poll_interval_secs: Some(self.base_config.poll_interval.as_secs()),
                runner_sleep_secs: Some(self.base_config.runner_sleep.as_secs()),
                python_bin: Some(self.base_config.sandbox.python_bin.clone()),
                timeout_secs: Some(self.base_config.sandbox.timeout.as_secs()),
                workspace_limit_mb: Some(self.base_config.sandbox.workspace_limit_bytes / 1024 / 1024),
                stdout_limit_mb: Some(self.base_config.sandbox.stdout_limit_bytes / 1024 / 1024),
                stderr_limit_mb: Some(self.base_config.sandbox.stderr_limit_bytes / 1024 / 1024),
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
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Newral Agent");
                    if self.running {
                        if ui.button("Stop").clicked() {
                            self.stop_agent();
                        }
                    } else if ui.button("Start").clicked() {
                        self.start_agent();
                    }
                });
            });

            egui::SidePanel::left("settings_panel").show(ctx, |ui| {
                ui.heading("Settings");
                ui.label("Node ID");
                ui.text_edit_singleline(&mut self.node_id);

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

                if let Some(err) = &self.last_error {
                    ui.colored_label(egui::Color32::RED, err);
                }
            });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Live Log");
                ui.separator();

                let lines = self.log_buffer.snapshot();
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for line in lines {
                        ui.label(line);
                    }
                });
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
