Portal Roadmap

Purpose
The portal is the public-facing window into Newral. MVP is HTTP-only with no auth, but the architecture anticipates secure access and role-based controls.

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
