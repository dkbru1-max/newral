Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Service Conventions (Newral)

Purpose
This document defines baseline conventions for all current and future Newral services so the system stays Kubernetes-ready while the MVP runs on Docker Compose.

Baseline Principles
- 12-factor: config via environment, stateless processes, logs to stdout/stderr.
- Services should be safe to restart at any time and rely on Postgres/Kafka/object storage for state.
- Each service must document its required env vars and defaults.

Configuration (Env Vars)
- All runtime configuration MUST be provided via environment variables.
- Root `.env.example` contains shared infra defaults; each service may include its own `.env.example` for service-specific settings.
- Never depend on local files for secrets or config; use env vars only.

Logging
- Logs go to stdout/stderr only (no local log files).
- Prefer structured logs with `level`, `ts`, `service`, and optional `trace_id` for correlation.

Health & Readiness
- Every HTTP service MUST expose:
  - `GET /healthz` for liveness
  - `GET /readyz` for readiness (fail if dependencies are not ready)
- Non-HTTP services should expose a minimal HTTP server for these endpoints.

Ports
- Default HTTP port: `8080` (override via `PORT` env var).
- If a service exposes gRPC, use `GRPC_PORT` (default `9090`) and document it.
- All ports must be explicit in Compose and future Kubernetes manifests.

Timeouts & Retries
- Client-side timeouts MUST be set for all outbound calls (DB, Kafka, HTTP/gRPC).
- Default timeouts should be sensible and configurable via env vars.
- Retries should be bounded with exponential backoff; avoid infinite retry loops.

Error Format
- HTTP APIs should return JSON errors:
  - `code` (string, stable)
  - `message` (human-readable)
  - `details` (optional structured data)
- Use consistent HTTP status codes (4xx for client errors, 5xx for server errors).

API Versioning
- External APIs should be versioned under `/api/v1`.
- Breaking changes require a new major version path (e.g., `/api/v2`).

Graceful Shutdown
- Services MUST handle SIGTERM: stop accepting new work, finish in-flight work where possible, and exit cleanly.
- Timeouts for shutdown should be configurable via env vars.
