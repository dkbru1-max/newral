Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

AI Modes

Overview
Newral supports four orchestration modes that control how the scheduler uses automation and AI-driven suggestions. Modes are expressed as `AI_MODE` and enforced by the Policy Engine.

Modes
AI_OFF
- Orchestration is fully deterministic.
- AI suggestions are ignored/denied by policy.
- Best for early MVP and safety-first operations.

AI_ADVISORY
- AI can suggest plans and optimizations.
- Human or deterministic rules still decide final actions.
- Policy Engine can allow or reject AI suggestions based on limits.

AI_ASSISTED
- AI may execute low-risk actions automatically (e.g., rechecks, reassignments).
- Higher-risk decisions still require policy approval or manual review.

AI_FULL
- AI can drive orchestration decisions end-to-end.
- Policy Engine still applies hard limits and budgets.

Configuration
Example config file: `config/config.example.yml`

Environment variables:
- `AI_MODE`: `AI_OFF` | `AI_ADVISORY` | `AI_ASSISTED` | `AI_FULL`
- `POLICY_MAX_CONCURRENT_TASKS`: max tasks per request (default 10)
- `POLICY_MAX_DAILY_BUDGET`: daily budget cap for orchestration decisions (default 100.0)
- `POLICY_RECHECK_THRESHOLD`: recheck ratio threshold (default 0.2)

Policy Engine is always the final gate for orchestration actions and writes an audit trail for every decision.
