Data Model (Conceptual)

Overview
This document describes the MVP data model in Postgres. It is a conceptual map of the tables created by `db/migrations/0001_init.sql` and should stay aligned with the migration files.

Tables
users
- Represents platform accounts.
- Fields: `id`, `email`, `password_hash`, `created_at`.

devices
- Represents registered devices (up to 5 per user).
- Fields: `id`, `user_id`, `device_uid`, `name`, `created_at`.
- Constraint: `UNIQUE (user_id, device_uid)`.
- Limit enforcement: trigger prevents more than 5 devices per `user_id`.

device_reputation
- Reputation state for each device.
- Fields: `device_id`, `score`, `updated_at`.
- One-to-one with `devices`.

projects
- Container for tasks and workflows.
- Fields: `id`, `name`, `description`, `status`, `created_at`.

tasks
- Units of work scheduled for execution.
- Fields: `id`, `project_id`, `status`, `payload`, `created_at`, `updated_at`.

task_results
- Outputs reported by devices.
- Fields: `id`, `task_id`, `device_id`, `status`, `result`, `created_at`.

flags
- Signals for Dr. Mann#n to review a user/device/task.
- Fields: `id`, `user_id`, `device_id`, `task_id`, `reason`, `details`, `created_at`.

Notes
- All services must read config from environment variables.
- Services must not auto-apply migrations; use `make migrate`.
