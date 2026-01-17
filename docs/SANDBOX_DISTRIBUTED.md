Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Sandbox Distributed

Overview
This document extends the MVP sandbox by introducing a distributed model that runs tasks on client agents and rechecks/aggregates results on the server. It follows the constraints in docs/PROJECT_BRIEF.md and builds on docs/SANDBOX_MVP.md.

Client Sandbox (Agent)
- Each agent executes Python code from task payloads inside a temporary workspace.
- Workspace guardrails remain: size limits, output limits, and hard timeouts.
- Agent returns a structured result payload with execution metadata (timestamps, byte sizes, hashes, exit code).
- Results are designed for server-side verification and aggregation.

Server Sandbox (Validator)
- A server-side sandbox re-runs agent code in a stricter environment.
- The server compares the agent result to its own execution (stdout hash + status).
- Audit flags are recorded for suspicious results, mismatches, and sandbox errors.
- AI validation is a heuristic layer that marks risky code or anomalous output.

Sharding and Aggregation
- Large tasks can be split into shards (group_id + parent_task_id).
- Each shard is executed independently by agents.
- The server aggregates shard outcomes into a single response.
- Aggregation is a first-class workflow step, not an ad-hoc script.

AI Validation Stage
- Before acceptance, results pass through AI heuristics.
- Heuristics mark risky code patterns, errors, or excessive output.
- Flagged results are stored in audit logs for review or re-execution.

Follow-up Task Planning
- After a group task completes, the scheduler can create a follow-up task.
- The MVP uses a simple rule-based planner (no ML dependency).
- Follow-ups are disabled via `DEMO_FOLLOWUP_ENABLED=0`.

Audit Logging
- All critical actions (recheck, AI flags, sandbox errors) are logged to the flags table.
- Audit records include task_id, device_id, decision, and diagnostics.
- This enables manual investigation and future AI training.

Security Posture (Current)
- Client sandbox does not provide strong OS isolation (see SANDBOX_MVP).
- Server sandbox runs in-process with strict time/output/workspace limits.
- No container-level isolation is enforced in MVP.

Planned Hardening
- Execute server rechecks inside isolated containers or microVMs.
- Enforce CPU/memory quotas via cgroups or job objects.
- Introduce syscall filtering and network sandboxing.
