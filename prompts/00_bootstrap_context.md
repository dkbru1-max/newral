Используй docs/PROJECT_BRIEF.md как единственный источник истины по проекту Newral (working title).
Если файла нет — создай его как шаблон с нужными разделами (архитектура, режимы AI, безопасность, роли, данные, деплой).

Задачи:
1) Сгенерировать docs/PLAN_MVP.md: вертикальный план MVP (сквозной срез) на 2–4 недели.
2) Сгенерировать docs/ADR/ADR-0001-compose-then-k8s.md: почему Docker Compose для MVP, но всё делаем Kubernetes-ready (12-factor, health endpoints, env config, migrations as jobs).
3) Сгенерировать docs/ADR/ADR-0002-messaging-kafka.md: Kafka как backbone для задач/событий, что идёт по темам.
4) Добавить Makefile с целями: up/down/ps/logs, fmt, test, lint (пока заглушки допустимы).
Никаких больших реализаций — только документы и каркас.
