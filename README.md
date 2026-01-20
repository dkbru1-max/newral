# Newral (working title)

## Repo structure
- `services/common`: shared startup helpers (tracing, env parsing, shutdown).
- `services/*/src`: split into `app`, `handlers`, `service`, `db`, `models`, `state`.
- `client/agent`: Rust agent with sandbox execution and GUI.
- `docs/REFACTORING_NOTES.md`: rationale for refactor decisions.
- `scripts/bpsw`: BPSW worker script used for distributed tasks.

## Local dev
1) Copy `.env.example` to `.env` and set passwords.
2) Start infrastructure: `docker compose up -d`
3) Apply migrations: `make migrate`
4) Check status: `docker compose ps`
5) Tail logs: `docker compose logs -f`
6) Stop: `docker compose down`

Kafka is exposed on host port `9092`, but inside the Docker network use `kafka:19092`.

## Local registry (offline pulls)
Use a local registry so Docker pulls images from `localhost:5000` instead of `docker.io`.

1) Start the local registry:
```bash
make registry-up
```

2) Sync external/base images into the local registry:
```bash
make registry-sync
```

3) Run the stack using the local-registry override:
```bash
docker compose -f docker-compose.yml -f docker-compose.local-registry.yml up -d --build
```

Re-run `make registry-sync` when you change image tags or want to refresh from `docker.io`.

Quality checks (per service Rust crates):
```bash
make fmt
make lint
make test
```

## Portal access (LAN)
The gateway publishes HTTP on port `80` so the portal is reachable from another machine.

1) Start the stack:
```bash
docker compose up -d --build
```
2) Open in a browser:
- `http://localhost/`
- `http://<VM_IP>/` from another machine on the same network

Firewall: allow inbound `80` (later `443` for HTTPS).

Quick check:
```bash
curl http://localhost/
```

## Service checks (curl)
Identity service:
```bash
curl -sS http://localhost:8081/healthz
curl -sS http://localhost:8081/readyz
curl -sS -X POST http://localhost:8081/v1/register -H 'Content-Type: application/json' -d '{"email":"dev@example.com","password":"dev"}'
curl -sS -X POST http://localhost:8081/v1/login -H 'Content-Type: application/json' -d '{"email":"dev@example.com","password":"dev"}'
curl -sS -X POST http://localhost:8081/v1/devices/register -H 'Content-Type: application/json' -d '{"user_id":"user-1","device_id":"device-1"}'
```

Scheduler service:
```bash
curl -sS http://localhost:8082/healthz
curl -sS http://localhost:8082/readyz
curl -sS -X POST http://localhost:8082/v1/tasks/request_batch -H 'Content-Type: application/json' -d '{"agent_uid":"00000000-0000-0000-0000-000000000000","max":3}'
curl -sS -X POST http://localhost:8082/v1/tasks/submit -H 'Content-Type: application/json' -d '{"task_id":"task-0000","result":"ok"}'
curl -sS -X POST http://localhost:8082/v1/agents/register -H 'Content-Type: application/json' -d '{"agent_uid":"00000000-0000-0000-0000-000000000000","hardware":{"cpu_model":"demo","ram_total_mb":1024}}'
curl -sS -X POST http://localhost:8082/v1/agents/metrics -H 'Content-Type: application/json' -d '{"agent_uid":"00000000-0000-0000-0000-000000000000","metrics":{"cpu_load":12.5}}'
curl -sS -X POST http://localhost:8082/v1/projects/bpsw/scripts/sync
curl -sS -X POST http://localhost:8082/v1/projects/bpsw/start -H 'Content-Type: application/json' -d '{"chunk_size":10000}'
```

Validator service:
```bash
curl -sS http://localhost:8083/healthz
curl -sS http://localhost:8083/readyz
curl -sS -X POST http://localhost:8083/v1/validate -H 'Content-Type: application/json' -d '{"task_id":1,"device_id":1,"result_hash":"deadbeef","outcome":"ok"}'
```

Telemetry service:
```bash
curl -sS http://localhost:8084/healthz
curl -sS http://localhost:8084/readyz
curl -sS -X POST http://localhost:8084/v1/event -H 'Content-Type: application/json' -d '{"event_type":"startup","payload":{"note":"hello"}}'
```

## Windows agent build (GitHub Actions)
Planned: CI workflow for Windows/Linux agent builds is not implemented yet.
