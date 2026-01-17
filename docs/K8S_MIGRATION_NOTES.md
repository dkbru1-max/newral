Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Kubernetes Migration Notes

Goal
Move from Docker Compose to Kubernetes without changing service code by relying on environment-based config and standard health endpoints.

Steps per service
1) Build and publish the container image (tagged version).
2) Create Deployment and Service manifests with:
   - `PORT` and service-specific env vars.
   - `/healthz` liveness and `/readyz` readiness probes.
3) Wire shared config via ConfigMaps and secrets (DB/Kafka/MinIO credentials).
4) Add resource limits/requests once baseline usage is known.

Infra migration
- Replace Compose services with managed equivalents or Helm charts:
  - Postgres
  - Kafka (KRaft)
  - MinIO or managed object storage
  - Redis (optional for MVP)

Data migration
- Restore backups (Postgres + MinIO) into the new cluster.
- Verify that services point to the new endpoints through env vars.

Operational checklist
- Add Ingress for external access (when needed).
- Add TLS and secret management (Kubernetes secrets or Vault).
- Add monitoring (Prometheus/Grafana) and logging aggregation.

Compose-to-K8s parity
- Env config stays identical between Compose and K8s.
- Health checks map to liveness/readiness probes.
- One service == one deployment.
