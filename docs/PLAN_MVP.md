# Newral MVP Plan (Vertical Slice, 2–4 Weeks)

## Scope (single source: `docs/PROJECT_BRIEF.md`)
This MVP is a thin, end-to-end slice that proves the core loop: submit a task, assign it to a node, execute, return result, persist state, and observe status. AI modes stay in **AI Off**; reliability and security are minimal but structured for growth.

## Goals
- Prove the end-to-end task lifecycle across orchestrator, agent, DB, and messaging.
- Establish Kubernetes-ready service boundaries while using Docker Compose for speed.
- Create a small, observable pipeline with health endpoints and basic metrics hooks.

## Non-goals (MVP)
- Full AI orchestration beyond **AI Off**.
- Advanced trust/reputation automation beyond placeholder fields.
- Complex DAG workflows beyond a single linear or standalone task.

## Vertical Slice (2–4 Weeks)
### Week 1: Foundations + Data Model
- Define service boundaries: Orchestrator, Agent, Trust (stub), API Gateway (stub), DB, Object Storage (placeholder), Kafka.
- Create minimal DB schema: nodes, tasks, task_runs, results, node_reputation.
- Define event contracts (Kafka topics) and HTTP endpoints (health, submit task, node heartbeat).
- Wire Compose for local dev; ensure services start and connect.

### Week 2: End-to-End Task Loop (AI Off)
- Implement orchestrator: receive task, enqueue, assign to node, track state.
- Implement agent: register/heartbeat, pull/consume assignment, execute dummy work, return result.
- Persist task states in PostgreSQL.
- Produce/consume minimal Kafka events for task assignment and completion.

### Week 3: Reliability Hooks + Observability
- Add health endpoints for all services.
- Add migrations (as jobs) and env-based config across services.
- Add basic logging and metrics stubs (counters, durations).
- Document runbook: local Compose startup, sample task submission, result inspection.

### Week 4 (Optional Buffer): Hardening + Docs
- Add basic failure handling: retries for transient failures, timeouts for tasks.
- Add lightweight trust fields and a stub trust update flow.
- Finalize MVP documentation (architecture, runbooks, API notes).

## Deliverables
- Running Compose environment with core services.
- End-to-end demo: submit task → node executes → result persists → status visible.
- Minimal docs: architecture sketch, runbook, event contracts.

## Risks / Mitigations
- **Kafka complexity**: keep event schema minimal; fallback to HTTP for bootstrap.
- **Agent variability**: use a single dummy task with deterministic output.
- **Scope creep**: enforce AI Off and single task type only.

## Exit Criteria
- One-click local demo (Compose) with visible task lifecycle.
- Task and node state persist across orchestrator restarts.
- Health endpoints for all core services.
