Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

# ADR-0001: Docker Compose for MVP, Kubernetes-Ready by Design

## Status
Accepted

## Context
The MVP needs a fast local developer loop and a stable, reproducible environment. The long-term deployment target is Kubernetes, but early iteration speed and low operational overhead are critical. The architecture requires multiple services (orchestrator, agents, DB, Kafka, object storage), which are easier to run locally with Docker Compose.

## Decision
Use **Docker Compose** for MVP development and demo. Design services and configs to be **Kubernetes-ready** from day one.

Kubernetes-ready here means:
- **12-factor configuration**: all runtime config via environment variables.
- **Health endpoints**: HTTP health/readiness probes for each service.
- **Stateless services**: service instances can be restarted without data loss.
- **Migrations as jobs**: schema migrations run as a discrete job step.
- **Externalized state**: DB/object storage are services with persistent volumes.

## Rationale
- Compose minimizes setup time and debugging friction for early development.
- Kubernetes introduces complexity (manifests, ingress, RBAC) that slows MVP iteration.
- Keeping services 12-factor and stateless makes the later switch to K8s straightforward.

## Consequences
- Local environment uses `docker-compose.yml` for orchestration.
- Service configs must avoid hardcoded paths/ports and rely on env vars.
- Health endpoints are mandatory, even in MVP.
- CI can run Compose for integration checks.

## Follow-up
- Create K8s manifests/Helm chart after MVP stabilization.
- Add readiness/liveness probes to all services.
