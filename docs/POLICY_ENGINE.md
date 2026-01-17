Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Policy Engine (Concept)

Goal
The Policy Engine is a deterministic decision layer that evaluates proposals and enforces platform limits regardless of AI mode.

Inputs
- Proposals from AI or deterministic rules (e.g., task assignment, retries, rechecks).
- Context metadata (device reputation, budgets, rate limits, project constraints).

Outputs
- allow | deny | limit
- Reasons list for auditability.

Audit Trail
- Every decision is logged with proposal metadata and the resulting action.
- Logs must include who/what proposed the action and which limits were applied.

Example Proposal Flow
1) Scheduler receives a task assignment proposal.
2) Policy Engine evaluates limits and mode.
3) Decision is returned to scheduler:
   - allow: proceed
   - limit: clamp parameters (e.g., max tasks)
   - deny: stop, record reason
