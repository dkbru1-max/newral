Deployment

Overview
Newral targets a Kubernetes production deployment while keeping the MVP development loop on Docker Compose. All services follow Kubernetes-ready conventions: env-based configuration, health endpoints, and clean shutdown behavior.

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
