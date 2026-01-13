Используй docs/PROJECT_BRIEF.md как источник истины.
Пиши, используя лучшие практики и изящные решения, никуда не спеши, делай всё аккуратно и проверяй код.

Нужно реализовать стратегию "DB + File/Object Storage":
1) docs/BACKUP_RESTORE.md:
   - что бэкапим (Postgres + MinIO buckets + конфиги)
   - как восстанавливаем на новой VM
   - принцип "программа сама поднимется из метаданных в БД" (в рамках MVP: compose + конфиг)
2) Добавь scripts/backup.sh и scripts/restore.sh (MVP):
   - pg_dump/pg_restore
   - экспорт/импорт MinIO через mc или простые копии volume (если проще для MVP)
3) Добавь Makefile цели backup/restore.

Цель: перенос MVP на другую машину понятной последовательностью команд.
