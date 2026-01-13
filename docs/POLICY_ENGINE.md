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
