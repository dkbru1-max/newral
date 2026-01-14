# Newral (working title)

## Local dev
1) Copy `.env.example` to `.env` and set passwords.
2) Start infrastructure: `docker compose up -d`
3) Apply migrations: `make migrate`
4) Check status: `docker compose ps`
5) Tail logs: `docker compose logs -f`
6) Stop: `docker compose down`

Kafka is exposed on host port `9092`, but inside the Docker network use `kafka:19092`.

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
curl -sS -X POST http://localhost:8082/v1/tasks/request -H 'Content-Type: application/json' -d '{"node_id":"node-1"}'
curl -sS -X POST http://localhost:8082/v1/tasks/submit -H 'Content-Type: application/json' -d '{"task_id":"task-0000","result":"ok"}'
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
Release build (recommended):
1) Create a tag locally:
```bash
git tag agent-v0.1.0
git push origin agent-v0.1.0
```
2) Wait for Actions run to finish.
3) Download from GitHub Releases: `newral-agent.exe` or `newral-agent-windows.zip`.

Manual build via Actions artifacts:
1) GitHub -> Actions -> Build Windows Agent -> Run workflow.
2) Download artifact `newral-agent-windows` from the completed run.
