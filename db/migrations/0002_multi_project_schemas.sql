BEGIN;

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS owner_id BIGINT REFERENCES users(id) ON DELETE SET NULL;

CREATE OR REPLACE FUNCTION sanitize_identifier(input TEXT)
RETURNS TEXT AS $$
BEGIN
    RETURN regexp_replace(lower(input), '[^a-z0-9_]+', '_', 'g');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION project_schema_name(p_id BIGINT, p_name TEXT)
RETURNS TEXT AS $$
DECLARE
    safe_name TEXT;
BEGIN
    safe_name := sanitize_identifier(COALESCE(p_name, 'project'));
    RETURN format('prj_%s_%s', p_id, safe_name);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION create_project_schema(p_id BIGINT, p_name TEXT)
RETURNS VOID AS $$
DECLARE
    schema_name TEXT;
BEGIN
    schema_name := project_schema_name(p_id, p_name);

    EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', schema_name);

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I.tasks (
            id BIGSERIAL PRIMARY KEY,
            status TEXT NOT NULL,
            payload JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )',
        schema_name
    );

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I.task_results (
            id BIGSERIAL PRIMARY KEY,
            task_id BIGINT NOT NULL REFERENCES %I.tasks(id) ON DELETE CASCADE,
            device_id BIGINT REFERENCES public.devices(id) ON DELETE CASCADE,
            status TEXT NOT NULL,
            result JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )',
        schema_name,
        schema_name
    );

    EXECUTE format(
        'CREATE INDEX IF NOT EXISTS idx_%s_tasks_status ON %I.tasks(status)',
        schema_name,
        schema_name
    );

    EXECUTE format(
        'CREATE INDEX IF NOT EXISTS idx_%s_task_results_task_id ON %I.task_results(task_id)',
        schema_name,
        schema_name
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION drop_project_schema(p_id BIGINT, p_name TEXT)
RETURNS VOID AS $$
DECLARE
    schema_name TEXT;
BEGIN
    schema_name := project_schema_name(p_id, p_name);
    EXECUTE format('DROP SCHEMA IF EXISTS %I CASCADE', schema_name);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION create_project_role(p_id BIGINT, p_name TEXT)
RETURNS VOID AS $$
DECLARE
    schema_name TEXT;
    role_name TEXT;
BEGIN
    schema_name := project_schema_name(p_id, p_name);
    role_name := format('%s_rw', schema_name);

    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
        EXECUTE format('CREATE ROLE %I', role_name);
    END IF;

    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', schema_name, role_name);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO %I', schema_name, role_name);
    EXECUTE format('ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO %I', schema_name, role_name);
END;
$$ LANGUAGE plpgsql;

COMMIT;
