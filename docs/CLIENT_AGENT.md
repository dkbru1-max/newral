Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Client Agent (MVP)

Purpose
The agent connects a volunteer device to the Newral scheduler. In MVP it:
- Sends periodic heartbeats.
- Requests batch tasks and manages a local queue.
- Executes Python tasks in a sandboxed workspace.
- Submits a result.

New in v0.2.0
- EULA gating on first launch (no network activity until accepted).
- Agent preferences per project/task type.
- Hardware inventory + periodic metrics (sysinfo; GPU best-effort via nvidia-smi).
- Local resource limits for CPU/RAM (GPU best-effort).

Configuration
The agent reads config from file and environment variables. Env vars override file settings.

Config file (example)
`client/agent/config.example.toml`

Environment variables
- `AGENT_CONFIG_PATH` (default: `client/agent/config.toml`)
- `NODE_ID` (default: `dev-node`)
- `SCHEDULER_URL` (default: `http://localhost:8082`)
- `HEARTBEAT_INTERVAL_SECS` (default: `10`)
- `POLL_INTERVAL_SECS` (default: `5`)
- `RUNNER_SLEEP_SECS` (default: `2`)

Security Principles
- Tasks are executed through a runner interface, designed for future sandbox isolation.
- No direct hardware access is required in MVP; compute is stubbed.
- All agent logs go to stdout/stderr.
- Secrets must be provided via env vars (never embedded in config files).
- The agent treats scheduler responses as untrusted input.

Sandbox Interface
The runner abstraction is the only execution path for tasks. Future work will replace the stub with a constrained sandbox (e.g., container-based or syscall-filtered runner).

Linux Install (simple)
1) Build: `cargo build --release -p newral-agent`
2) Copy binary to `/usr/local/bin/newral-agent`.
3) Place config at `client/agent/config.toml` or set env vars.
4) Run: `newral-agent`

Windows Install (simple)
1) Build: `cargo build --release -p newral-agent`
2) Copy `target\release\newral-agent.exe` to a folder in PATH.
3) Create `client\agent\config.toml` or set env vars in PowerShell.
4) Run: `newral-agent.exe`
