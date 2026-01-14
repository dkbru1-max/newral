Используй docs/PROJECT_BRIEF.md и текущую модель данных. Сделай поддержку проектов так, чтобы данные задач/результатов жили в отдельных схемах.

Цель: в одной БД PostgreSQL:
- public: users, devices, projects (метаданные)
- schema per project: tasks, task_results (+ при необходимости flags)

Сделай:
1) Миграции:
   - Таблица projects в public (name уникальное, owner_id, created_at).
   - При создании проекта создается schema: prj_<id> (или имя + id), чтобы избежать коллизий.
   - В схеме создаются tasks/task_results с нужными индексами.
   - Реализуй безопасную генерацию имен (только [a-z0-9_], без SQL-инъекций).

2) Права:
   - Для MVP достаточно одной сервисной учетки БД (текущей).
   - Но добавь код/команды, которые умеют создать role prj_<id>_rw и выдать ей права только на schema проекта (на будущее).

3) API:
   - Добавь эндпойнты:
     GET /v1/projects
     POST /v1/projects  (name, description, owner_id)
     DELETE /v1/projects/{id}
   - При POST: создать запись projects + schema + таблицы.
   - При DELETE: DROP SCHEMA ... CASCADE + удалить запись projects.

4) Обнови scheduler/validator так, чтобы при работе с задачами учитывалась схема проекта.
   - В MVP можно: каждая задача имеет project_id, а запросы идут в schema проекта.
   - Для SQL: либо квалифицируй таблицы schema.tasks, либо выставляй search_path на время транзакции.

5) Документация:
   - docs/MULTI_PROJECT.md: схемы vs отдельные базы, плюсы/минусы, как перейти позже.
