use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf, time::Duration};
use tokio::time::sleep;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Debug, Clone)]
struct AgentConfig {
    node_id: String,
    scheduler_url: String,
    heartbeat_interval: Duration,
    poll_interval: Duration,
    runner_sleep: Duration,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    node_id: Option<String>,
    scheduler_url: Option<String>,
    heartbeat_interval_secs: Option<u64>,
    poll_interval_secs: Option<u64>,
    runner_sleep_secs: Option<u64>,
}

#[derive(Debug, Clone)]
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

#[derive(Deserialize)]
struct TaskResponse {
    status: String,
    task_id: String,
    policy_decision: String,
    granted_tasks: u32,
    reasons: Vec<String>,
}

#[derive(Serialize)]
struct TaskSubmitRequest<'a> {
    task_id: &'a str,
    result: &'a str,
}

#[derive(Deserialize)]
struct TaskSubmitResponse {
    status: String,
}

#[derive(Debug)]
struct Task {
    id: String,
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

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
        runner: std::sync::Arc::new(SleepRunner {}),
    };

    // Heartbeat runs in a separate task.
    let heartbeat_agent = agent.clone();
    tokio::spawn(async move {
        heartbeat_agent.heartbeat_loop().await;
    });

    // Main loop polls for tasks and submits results.
    agent.run_loop().await;
}

impl Agent {
    async fn heartbeat_loop(self) {
        loop {
            if let Err(err) = self.send_heartbeat().await {
                tracing::warn!(error = %err, "heartbeat failed");
            }
            sleep(self.config.heartbeat_interval).await;
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

    async fn run_loop(&self) {
        loop {
            match self.request_task().await {
                Ok(Some(task)) => {
                    // Runner abstracts future sandboxed execution.
                    let result = self
                        .runner
                        .run(&task, self.config.runner_sleep)
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

            sleep(self.config.poll_interval).await;
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
        if body.policy_decision == "deny" || body.granted_tasks == 0 {
            // Policy denies or clamps to zero tasks.
            tracing::info!(reasons = ?body.reasons, "policy denied or limited to zero");
            return Ok(None);
        }

        Ok(Some(Task { id: body.task_id }))
    }

    async fn submit_result(&self, task: &Task, result: &str) -> Result<(), reqwest::Error> {
        let url = format!("{}/v1/tasks/submit", self.config.scheduler_url);
        let response = self
            .client
            .post(url)
            .json(&TaskSubmitRequest {
                task_id: &task.id,
                result,
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

trait TaskRunner {
    fn name(&self) -> &'static str;
    fn run<'a>(
        &'a self,
        task: &'a Task,
        sleep_duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>>;
}

struct SleepRunner;

impl TaskRunner for SleepRunner {
    fn name(&self) -> &'static str {
        "sleep"
    }

    fn run<'a>(
        &'a self,
        task: &'a Task,
        sleep_duration: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
        Box::pin(async move {
            // Stubbed execution path for MVP.
            tracing::info!(task_id = %task.id, runner = self.name(), "running task");
            sleep(sleep_duration).await;
            "ok".to_string()
        })
    }
}

fn load_config() -> Result<AgentConfig, String> {
    // Default path is inside the repo for local dev.
    let config_path = env::var("AGENT_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("client/agent/config.toml"));

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

    Ok(AgentConfig {
        node_id,
        scheduler_url,
        heartbeat_interval: Duration::from_secs(heartbeat_interval),
        poll_interval: Duration::from_secs(poll_interval),
        runner_sleep: Duration::from_secs(runner_sleep),
    })
}
