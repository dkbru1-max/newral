Multi-Project Storage Strategy

Overview
Newral stores shared metadata in the `public` schema and isolates project workloads in per-project schemas within the same Postgres database.

Schema Layout
- `public`: users, devices, projects, device_reputation, flags.
- `prj_<id>_<name>`: tasks, task_results for each project.

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
