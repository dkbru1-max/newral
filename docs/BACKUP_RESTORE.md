Update notes (v0.2.0)
- Project isolation: GUID per project, separate Postgres schemas, MinIO prefixes.
- BPSW workflow: script sync/start, hash verification, task types, real-range defaults.
- Agent: EULA gate, batch tasks, preferences, metrics via sysinfo, local limits.
- Portal: SPA navigation, breadcrumbs, BPSW controls, version display.
- Builds: Rust 1.88 base images for aws-sdk compatibility.
- Known gaps: BPSW DET pipeline, portal detail pages on mock data, agent CI workflow.

Backup and Restore (MVP)

Scope
The MVP backup set includes:
- Postgres database (metadata, users, tasks, flags, reputation).
- MinIO object storage data (`/data` volume).
- Configuration files: `.env`, `docker-compose.yml`, and `config/config.example.yml`.

Principle
The platform should be able to come back on a new VM using:
- Postgres metadata as the source of truth.
- MinIO object data restored to the same buckets.
- Docker Compose configuration for infrastructure.

Backup (local)
1) Ensure `docker compose up -d` is running.
2) Run:
   - `make backup`

Artifacts are stored under `backups/<timestamp>/`.

Restore (new VM)
1) Copy the backup directory to the new machine.
2) Restore configuration files:
   - Place `.env` and `docker-compose.yml` in the repo root.
3) Start infrastructure:
   - `docker compose up -d`
4) Restore data:
   - `make restore BACKUP=backups/<timestamp>`

Notes
- Restore overwrites Postgres data and MinIO volume contents.
- For production, prefer dedicated backup tooling and object storage replication; this MVP uses local volume exports.
