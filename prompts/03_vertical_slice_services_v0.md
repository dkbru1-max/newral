Используй docs/PROJECT_BRIEF.md как источник истины.
Пиши, используя лучшие практики и изящные решения, никуда не спеши, делай всё аккуратно и проверяй код.

Сделай минимальный вертикальный срез сервисов (Rust), чтобы они:
- собирались,
- запускались как контейнеры,
- имели /healthz и /readyz,
- читали конфиг из env,
- корректно завершались по SIGTERM.

Сервисы:
1) services/identity-service
2) services/scheduler-service
3) services/validator-service
4) services/telemetry-service

Функционал пока заглушечный, но endpoints должны существовать (например REST):
- identity: POST /v1/register, POST /v1/login, POST /v1/devices/register (лимит 5 устройств)
- scheduler: POST /v1/tasks/request, POST /v1/tasks/submit
- validator: POST /v1/validate
- telemetry: POST /v1/event

Добавь в docker-compose.yml эти сервисы как контейнеры, чтобы они стартовали вместе с инфраструктурой.
Добавь README: команды curl для проверки.
