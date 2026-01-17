Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Deployment

Overview
Newral targets a Kubernetes production deployment while keeping the MVP development loop on Docker Compose. All services follow Kubernetes-ready conventions: env-based configuration, health endpoints, and clean shutdown behavior.

Current build notes
- Rust services build with `rust:1.88-bookworm` images (aws-sdk dependency requirement).
- Frontend build copies `VERSION` into `/version.txt` for UI version display.

Development (Docker Compose)
1) Copy `.env.example` to `.env` and set passwords.
2) Start infra services:
   - `make up`
3) Inspect status:
   - `make ps`
4) Tail logs:
   - `make logs`
5) Stop everything:
   - `make down`

Compose provides the core infrastructure services (Postgres, Kafka, object storage) needed for the MVP. Application services will be added as they are implemented.

Production (Kubernetes, later)
Kubernetes manifests and Helm chart skeletons live in:
- `deploy/k8s/`
- `deploy/helm/`

When services are added, each one must be deployable on Kubernetes without code changes by following the conventions in `docs/SERVICE_CONVENTIONS.md`.
