Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Portal Roadmap

Purpose
The portal is the public-facing window into Newral. MVP is HTTP-only with no auth, but the architecture anticipates secure access and role-based controls.

Current implementation (v0.2.0)
- SPA layout with breadcrumbs and drill-down routes.
- Home page is a single-scroll dashboard with anchor navigation on the left.
- Projects page includes BPSW script sync + start controls.
- Version is shown in the sidebar, topbar, and footer (from `VERSION`).

Next steps (post-MVP)
1) HTTPS
- Terminate TLS at gateway (port 443).
- Use managed certificates (ACME or cloud load balancer).

2) Authentication
- Add auth proxy or integrate identity service (OIDC/OAuth).
- Enforce login for non-public sections.

3) RBAC
- Roles: admin, operator, investor, project owner.
- Limit access per section (e.g., tasks/agents for ops only).

4) Observability
- Real metrics dashboards (Prometheus/Grafana or custom).
- Alerting on service health and SLA breaches.

5) Data wiring
- Replace placeholder UI data with API calls.
- Add pagination and filters for agents/tasks/projects.
