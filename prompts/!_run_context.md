# Newral quick context (for reloading)

## Version
- Platform version: 0.2.0
- Status snapshot: 2026-01-20

## Project summary
Newral is a distributed compute platform built as a microservice stack. The MVP covers task orchestration, agent sandbox execution, server-side validation, a demo wordcount workflow, and a BPSW project workflow with script distribution and hash verification. The stack runs via Docker Compose and is designed to be Kubernetes-ready.

Core pillars:
- Scheduler service assigns tasks, enforces policy, manages project isolation, and streams live summaries.
- Agent executes Python payloads in a sandboxed workspace and reports structured results.
- Validator re-runs tasks server-side for recheck and AI heuristics.
- Portal provides a SPA admin console with breadcrumbs and drill-down navigation.
- Postgres + MinIO are the storage backbone; Kafka and Redis are in the compose stack.

## Architecture highlights (current implementation)
- Project isolation:
  - Each project has a GUID and its own Postgres schema (`prj_<guid>`).
  - Object storage uses per-project prefixes in a shared MinIO bucket.
  - Demo project is flagged `is_demo` and protected from deletion.
- BPSW workflow:
  - Script stored in repo (`scripts/bpsw/bpsw_worker.py`) and uploaded to MinIO.
  - Task payloads carry script URL + SHA-256, agent verifies hash before running.
  - Task types: main_odds, large_numbers, chernick, pomerance_lite, pomerance_modular, lambda_plus_one.
  - Real-range defaults are enabled when no explicit start/end is provided.
- Agent runtime:
  - EULA gating: agent does nothing until accepted.
  - Batch task requests and local queue.
  - Preferences per project/task type are sent to scheduler.
  - Periodic metrics sent via sysinfo (GPU via nvidia-smi best effort).
  - Local CPU/RAM throttling enforced; GPU throttling depends on available metrics.
- Admin portal:
  - SPA routing with breadcrumbs and back button.
  - Projects page includes BPSW script sync and project start controls.

## Repo structure (high level)
- `services/` Rust microservices:
  - `services/scheduler-service/`
  - `services/validator-service/`
  - `services/identity-service/`
  - `services/telemetry-service/`
  - `services/common/` (shared utilities)
- `client/agent/`: Rust agent (CLI + GUI via feature flag), sandbox execution.
- `frontend/`: Portal UI (React + custom CSS).
- `gateway/`: NGINX reverse proxy.
- `db/`: SQL migrations.
- `docs/`: design + ADRs + sandbox docs.
- `docker-compose.yml`: local stack.
- `scripts/bpsw/`: BPSW worker script for distributed tasks.

## Critical files to review first
1) `docs/PROJECT_BRIEF.md`
   - Vision, architecture, and roadmap with current implementation status header.
2) `docs/SANDBOX_MVP.md` and `docs/SANDBOX_DISTRIBUTED.md`
   - MVP and distributed sandbox architecture.
3) `services/scheduler-service/src/`
   - `main.rs` boot
   - `app.rs` routes
   - `handlers.rs` HTTP endpoints
   - `service.rs` business logic (demo flow, BPSW flow, SSE summary)
   - `db.rs` SQL helpers
   - `policy.rs` AI mode / task request policy
   - `storage.rs` MinIO client
4) `services/validator-service/src/`
   - `handlers.rs` validate / recheck / aggregate
   - `sandbox.rs` server sandbox execution
   - `ai.rs` heuristic checks
   - `db.rs` DB helpers
5) `client/agent/src/main.rs`
   - agent runtime, sandbox execution, GUI (feature `gui`), EULA, metrics, limits.
6) `gateway/nginx.conf`
   - proxy rules, SSE config for scheduler stream.
7) `frontend/src/App.jsx`
   - SPA routing, BPSW actions, SSE live data.
8) `docker-compose.yml`
   - service wiring, ports, env.

## Stack
- Rust microservices (axum, tokio, tokio-postgres, tracing, aws-sdk-s3).
- Agent: Rust, optional GUI via `eframe/egui` (feature `gui`).
- Frontend: React (Vite), custom CSS.
- Infra: Postgres, Redis, Kafka, MinIO, NGINX gateway.
- Deployment: Docker Compose now, Kubernetes-ready patterns (12-factor, health endpoints, env config).
- Builders: Dockerfiles use Rust 1.88 to satisfy aws-sdk dependencies.

## Service ports (compose)
- Gateway: `80` -> portal + API proxy
- Identity: `8081`
- Scheduler: `8082`
- Validator: `8083`
- Telemetry: `8084`

## API highlights
Scheduler:
- `POST /v1/tasks/request_batch` (agent batch requests)
- `POST /v1/tasks/submit`
- `GET /v1/summary` + `GET /v1/stream` (SSE)
- `POST /v1/agents/register`
- `POST /v1/agents/metrics`
- `POST /v1/agents/preferences`
- `POST /v1/projects/bpsw/scripts/sync`
- `POST /v1/projects/bpsw/start`

Validator:
- `POST /v1/recheck` / `POST /v1/validate`

Demo wordcount:
- `POST /v1/demo/wordcount/start?parts=5`
- `GET /v1/demo/wordcount/status`
- `GET /v1/demo/wordcount/result`

## Useful commands
- Build & run: `docker compose up -d --build`
- Local registry:
  - Start registry: `make registry-up`
  - Sync images: `make registry-sync`
  - Run stack with local images: `docker compose -f docker-compose.yml -f docker-compose.local-registry.yml up -d --build`
- Migrations: `make migrate`
- Lint/test: `make fmt && make lint && make test`

## Known limitations (MVP)
- Client sandbox is process-based only (no container/VM isolation).
- Server sandbox runs in-process with time/output/workspace limits.
- BPSW deterministic proof (DET pool) not implemented.
- Admin portal detail pages use mock data (API wiring pending).
- Agent CI workflow for Windows/Linux not yet implemented.

---

## Status snapshot (2026-01-20)
- Compose builds with Rust 1.88 images.
- Scheduler supports project GUID schemas and MinIO prefixes.
- BPSW workflow wired (script sync + start) with real-range defaults.
- Agent supports EULA, batch tasks, preferences, metrics, and local limits.
- Portal is SPA with breadcrumbs and BPSW controls.
- Local Docker registry is used to avoid pulling base images on each run (`localhost:5000` + `docker-compose.local-registry.yml`).

## Done / not done (and why)
Done:
- Project GUID isolation with per-project schemas and MinIO prefixes.
- BPSW script distribution with hash verification and task-type filtering.
- Agent EULA gating, batch tasks, preferences, metrics, and local resource limits.
- Portal SPA navigation with drill-down pages and BPSW controls.

Not done:
- BPSW deterministic proof (DET pool): deferred for MVP to keep runtime small and cross-platform.
- Portal detail pages wired to real backend data: currently using mock data to unblock UI work.
- Agent CI workflow: Windows agent release workflow exists in GitHub Actions (`agent-windows.yml`), Linux pipeline still pending.
