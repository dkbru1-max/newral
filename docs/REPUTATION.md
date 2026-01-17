Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Reputation (MVP)

Purpose
Reputation is a device-level score used by the scheduler and Dr. Mann#n to reduce risk from unreliable nodes during MVP.

Scoring Rules
- OK result: +1
- needs_recheck: -1
- suspicious: -5

Suspicion Thresholds
- Score <= -10: device is considered low-reputation.
- Any single suspicious result immediately marks the device for review.

Dr. Mann#n Flags
Flags are stored in `flags` table and reference device/task when possible.
- `suspicious_result`: created when a device reports a result classified as suspicious.
- `low_reputation`: created when a device's reputation score drops to -10 or lower.

Scheduler Impact
When a device is flagged or low-reputation:
- Assign lower priority or isolate to low-risk tasks.
- Enforce mandatory recheck by independent devices.
- Optionally rate-limit how often the device receives tasks.

Validator Behavior (MVP Stub)
The validator service:
- Accepts results via `POST /v1/validate`.
- Classifies outcomes into `ok`, `needs_recheck`, or `suspicious`.
- Writes reputation deltas into `device_reputation`.
- Writes flags for suspicious outcomes and low-reputation thresholds.
- Logs every decision with task_id/device_id/decision for audit.
