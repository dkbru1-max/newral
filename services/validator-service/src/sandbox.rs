use sha2::{Digest, Sha256};
use std::{
    env,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{io::AsyncReadExt, process::Command, time::sleep};

use crate::models::{ServerSandboxResult, TaskPayload};
use crate::state::ServerSandboxConfig;

struct ExecutionOutput {
    status: String,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    exit_code: Option<i32>,
    error: Option<String>,
}

pub async fn run_server_sandbox(
    payload: &TaskPayload,
    sandbox: &ServerSandboxConfig,
) -> Result<ServerSandboxResult, String> {
    let script = payload
        .script
        .as_deref()
        .ok_or_else(|| "missing script".to_string())?;
    let script_hash = hash_bytes(script.as_bytes());
    let workspace = create_workspace("server")?;
    write_inputs(&workspace, payload.inputs.as_ref())?;
    write_script(&workspace, script)?;
    if dir_size(&workspace) > sandbox.workspace_limit_bytes {
        return Err("workspace limit exceeded".to_string());
    }

    let started_at = SystemTime::now();
    let output = execute_python(&workspace, sandbox).await?;
    if dir_size(&workspace) > sandbox.workspace_limit_bytes {
        return Err("workspace limit exceeded".to_string());
    }
    let ended_at = SystemTime::now();

    let stdout_hash = hash_bytes(output.stdout.as_bytes());

    Ok(ServerSandboxResult {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
        duration_ms: output.duration_ms,
        started_at_ms: started_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        ended_at_ms: ended_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        exit_code: output.exit_code,
        error: output.error,
        stdout_sha256: stdout_hash,
        script_sha256: script_hash,
    })
}

async fn execute_python(
    workspace: &Path,
    sandbox: &ServerSandboxConfig,
) -> Result<ExecutionOutput, String> {
    let mut command = Command::new(sandbox.python_bin.as_str());
    command.arg("-I").arg("task.py");
    command.current_dir(workspace);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let started_at = SystemTime::now();
    let mut child = command.spawn().map_err(|err| format!("spawn: {err}"))?;
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("stderr unavailable")?;

    let stdout_handle = tokio::spawn(read_limited(stdout, sandbox.stdout_limit_bytes));
    let stderr_handle = tokio::spawn(read_limited(stderr, sandbox.stderr_limit_bytes));

    let status = tokio::select! {
        result = child.wait() => {
            result.map_err(|err| format!("wait: {err}"))?
        }
        _ = sleep(sandbox.timeout) => {
            let _ = child.kill().await;
            stdout_handle.abort();
            stderr_handle.abort();
            return Ok(ExecutionOutput {
                status: "timeout".to_string(),
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration_ms: sandbox.timeout.as_millis() as u64,
                exit_code: None,
                error: Some("timeout".to_string()),
            });
        }
    };

    let stdout_bytes = stdout_handle
        .await
        .map_err(|_| "stdout join error".to_string())??;
    let stderr_bytes = stderr_handle
        .await
        .map_err(|_| "stderr join error".to_string())??;

    let duration_ms = SystemTime::now()
        .duration_since(started_at)
        .unwrap_or_default()
        .as_millis() as u64;
    let stdout_text = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).to_string();

    if !status.success() {
        return Ok(ExecutionOutput {
            status: "error".to_string(),
            stdout: stdout_text.trim().to_string(),
            stderr: stderr_text.clone(),
            duration_ms,
            exit_code: status.code(),
            error: Some(format!("exit: {status}, stderr: {stderr_text}")),
        });
    }

    Ok(ExecutionOutput {
        status: "ok".to_string(),
        stdout: stdout_text.trim().to_string(),
        stderr: stderr_text.trim().to_string(),
        duration_ms,
        exit_code: status.code(),
        error: None,
    })
}

async fn read_limited<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    limit_bytes: u64,
) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|err| format!("read: {err}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() as u64 > limit_bytes {
            return Err("output limit exceeded".to_string());
        }
    }
    Ok(buffer)
}

fn create_workspace(prefix: &str) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock error".to_string())?
        .as_millis();
    let dir_name = format!("newral_server_{}_{}", prefix, timestamp);
    let workspace = env::temp_dir().join(dir_name);
    std::fs::create_dir_all(&workspace).map_err(|err| format!("create workspace: {err}"))?;
    Ok(workspace)
}

fn write_inputs(
    workspace: &Path,
    inputs: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), String> {
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
    let path = workspace.join("task.py");
    std::fs::write(path, script).map_err(|err| format!("write script: {err}"))?;
    Ok(())
}

fn is_safe_filename(name: &str) -> bool {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn dir_size(path: &Path) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::is_safe_filename;

    #[test]
    fn rejects_path_traversal() {
        assert!(!is_safe_filename("../secret.txt"));
        assert!(!is_safe_filename("..\\secret.txt"));
        assert!(is_safe_filename("input.txt"));
    }
}
