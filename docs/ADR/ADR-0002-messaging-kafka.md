Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

# ADR-0002: Kafka as the Messaging Backbone

## Status
Accepted

## Context
Newral is an event-driven distributed system with asynchronous task assignment, completion, and trust updates across multiple services. We need a durable, scalable message backbone that supports partitioned consumption, replay, and decoupling between producers and consumers.

## Decision
Adopt **Apache Kafka** as the core messaging backbone for tasks, events, and telemetry.

## Rationale
- Kafka provides durable, ordered logs with replay.
- Consumer groups allow horizontal scaling.
- Decouples services and keeps system resilient to partial outages.
- Fits the platform architecture defined in `docs/PROJECT_BRIEF.md`.

## Topics (initial)
- `tasks.created`: new task submissions from API or orchestrator.
- `tasks.assigned`: task assignment to a node (partition by node_id).
- `tasks.completed`: node execution results and status.
- `nodes.heartbeat`: node health and capacity updates.
- `trust.updated`: reputation score updates and validation results.
- `metrics.events`: lightweight metrics/events for observability.

## Consequences
- Services must implement idempotent processing (Kafka delivers at-least-once).
- Topic naming and schema evolution rules are required.
- A schema registry is optional for MVP but should be considered post-MVP.

## Follow-up
- Define a minimal event schema per topic.
- Implement producer/consumer stubs in orchestrator and agent.
- Add retry and dead-letter patterns if needed.
