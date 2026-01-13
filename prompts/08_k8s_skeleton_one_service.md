Используй docs/PROJECT_BRIEF.md как источник истины.
Пиши, используя лучшие практики и изящные решения, никуда не спеши, делай всё аккуратно и проверяй код.

Создай в deploy/k8s/ минимальные манифесты как шаблон (не для прод, а как каркас):
- namespace newral
- deployment+service для identity-service
- configmap/secret примеры (плейсхолдеры)
- liveness/readiness probes на /healthz и /readyz

Добавь docs/K8S_MIGRATION_NOTES.md: какие шаги нужны, чтобы перевести остальные сервисы.
Задача: показать путь Compose -> K8s без переписывания.
