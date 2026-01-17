Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Multi-Project Storage Strategy

Overview
Newral stores shared metadata in the `public` schema and isolates project workloads in per-project schemas within the same Postgres database.

Schema Layout
- `public`: users, devices, projects, device_reputation, flags.
- `prj_<guid>`: tasks, task_results for each project (GUID-based schema names).

Object storage
- MinIO uses a shared bucket with per-project prefixes (`storage_prefix`).

Why schemas (MVP)
Pros
- Clear separation of project data with a single database to operate.
- Easy to drop a project (`DROP SCHEMA ... CASCADE`).
- Future-ready for per-project roles and grants.

Cons
- Harder to scale storage independently per project.
- Cross-project analytics require explicit schema qualification.

Future options
- Separate database per project for stronger isolation and scaling.
- Managed multi-tenant storage with read replicas per project.

Role model (future)
The migration provides `create_project_role(p_id, p_name)` which creates `prj_<id>_<name>_rw` and grants it access only to that schema.
This allows moving to least-privilege DB accounts later without changing schema layout.
