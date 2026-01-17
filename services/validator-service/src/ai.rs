use crate::models::{ServerSandboxResult, TaskPayload};

pub fn inspect(payload: &TaskPayload, result: &ServerSandboxResult) -> Option<String> {
    let script = if let Some(script) = payload.script.as_deref() {
        script
    } else if payload.script_url.is_some() {
        ""
    } else {
        return Some("missing_script".to_string());
    };
    let risky_tokens = [
        "import os",
        "import subprocess",
        "socket",
        "requests",
        "open(",
        "shutil",
        "pathlib",
        "__import__",
        "eval(",
    ];
    if risky_tokens.iter().any(|token| script.contains(token)) {
        return Some("risky_code_pattern".to_string());
    }
    if result.status != "ok" {
        return Some("execution_error".to_string());
    }
    if result.stdout.len() > 10000 {
        return Some("stdout_too_large".to_string());
    }
    None
}
