# Newral quick context (for reloading)

## Project summary
Newral is a distributed compute platform (working title) built as a microservice stack. It runs tasks on agents, validates results on server, and exposes a portal/dashboard. MVP focuses on a vertical slice: task orchestration, agent sandbox, validation, and a demo wordcount workflow.

Key pillars:
- Scheduler service assigns tasks and tracks status.
- Agent executes Python payloads in a sandboxed workspace and reports structured results.
- Validator re-runs tasks server-side for recheck and AI heuristics.
- Kafka + Postgres + MinIO are the infra backbone (Compose now, K8s‑ready later).

## Repo structure (high level)
- `services/` Rust microservices:
  - `services/scheduler-service/`
  - `services/validator-service/`
  - `services/identity-service/`
  - `services/telemetry-service/`
  - `services/common/` (shared utilities)
- `client/agent/`: Rust agent (CLI + GUI via feature flag), sandbox execution.
- `frontend/`: Portal UI.
- `gateway/`: NGINX reverse proxy.
- `db/`: SQL migrations.
- `docs/`: design + ADRs + sandbox docs.
- `docker-compose.yml`: local stack.

## Critical files to review first
1) `docs/PROJECT_BRIEF.md`
   - Single source of truth for vision, architecture, AI modes, security, etc.
2) `docs/SANDBOX_MVP.md` and `docs/SANDBOX_DISTRIBUTED.md`
   - MVP and distributed sandbox architecture.
3) `services/scheduler-service/src/` (layered)
   - `main.rs` boot
   - `app.rs` routes
   - `handlers.rs` HTTP endpoints
   - `service.rs` business logic (demo flow, SSE summary)
   - `db.rs` SQL helpers
   - `policy.rs` AI mode / task request policy
4) `services/validator-service/src/` (layered)
   - `handlers.rs` validate / recheck / aggregate
   - `sandbox.rs` server sandbox execution
   - `ai.rs` heuristic checks
   - `db.rs` DB helpers
5) `client/agent/src/main.rs`
   - agent runtime, sandbox execution, structured result payload, GUI (feature `gui`).
6) `gateway/nginx.conf`
   - proxy rules, SSE config for scheduler stream.
7) `frontend/src/App.jsx`
   - SSE live data, portal UI.
8) `docker-compose.yml`
   - service wiring, ports, env.

## Stack
- Rust microservices (axum, tokio, tokio-postgres, tracing).
- Agent: Rust, optional GUI via `eframe/egui` (feature `gui`).
- Frontend: React (Vite), custom CSS.
- Infra: Postgres, Redis, Kafka, MinIO, NGINX gateway.
- Deployment: Docker Compose now, Kubernetes‑ready patterns (12‑factor, health endpoints, env config).

## Current state
- Repo refactor done: services are split into `app/handlers/service/db/models/state` and shared startup helpers live in `services/common`.
- Scheduler has live SSE endpoints: `/v1/summary` and `/v1/stream`.
- Demo wordcount flow:
  - `/v1/demo/wordcount/start` creates group + shard tasks.
  - `/v1/demo/wordcount/status` and `/v1/demo/wordcount/result` show progress.
  - Follow‑up task planning exists (rule‑based, optional via `DEMO_FOLLOWUP_ENABLED=0`).
- Agent returns JSON result payload with metadata (stdout/stderr, hashes, duration, workspace size, etc.).
- Validator can recheck tasks server‑side and aggregate shard results; includes AI heuristic flags and audit log insertions.
- Gateway is configured for SSE (no buffering, long read timeout).
- Tests and lint: `make fmt`, `make lint`, `make test` succeed.
- Docker Compose build context is repo root (needed to include `services/common`).

## Service ports (compose)
- Gateway: `80` -> portal + API proxy
- Identity: `8081`
- Scheduler: `8082`
- Validator: `8083`
- Telemetry: `8084`

## Useful commands
- Build & run: `docker compose up -d --build`
- Migrations: `make migrate`
- Lint/test: `make fmt && make lint && make test`

## Known limitations (MVP)
- Client sandbox is process‑based only (no container/VM isolation).
- Server sandbox runs in‑process with time/output/workspace limits; no VM.
- Some DTO fields are unused placeholders (kept for forward compatibility).
- Telemetry/identity are minimal stubs.


---

## Status snapshot (2026-01-15 19:03 MSK)
- Portal: running via gateway on port 80; UI loads and uses SSE updates from scheduler.
- Live data: scheduler `/v1/stream` and `/v1/summary` provide live counts; frontend consumes them.
- Demo workflow: wordcount demo endpoints are wired; start/status/result work (with follow-up task enabled by default).
- Agent: builds and runs; executes python tasks in sandbox and returns structured JSON results.
- Validator: server sandbox recheck + aggregate endpoints available; AI heuristic flags enabled (can be disabled).
- Compose: `docker compose up -d --build` completes successfully.

