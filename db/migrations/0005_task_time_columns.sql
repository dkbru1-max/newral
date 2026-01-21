BEGIN;

ALTER TABLE tasks
    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ;

CREATE OR REPLACE FUNCTION ensure_task_time_columns(schema_name TEXT)
RETURNS VOID AS $$
BEGIN
    EXECUTE format(
        'ALTER TABLE %I.tasks ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ',
        schema_name
    );
    EXECUTE format(
        'ALTER TABLE %I.tasks ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ',
        schema_name
    );
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    rec RECORD;
    schema_name TEXT;
BEGIN
    FOR rec IN SELECT id, name, guid FROM projects LOOP
        schema_name := project_schema_name(rec.id, rec.name, rec.guid);
        PERFORM ensure_task_time_columns(schema_name);
    END LOOP;
END $$;

DROP FUNCTION ensure_task_time_columns(TEXT);

CREATE OR REPLACE FUNCTION create_project_schema(p_id BIGINT, p_name TEXT, p_guid UUID)
RETURNS VOID AS $$
DECLARE
    schema_name TEXT;
BEGIN
    schema_name := project_schema_name(p_id, p_name, p_guid);

    EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', schema_name);

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I.tasks (
            id BIGSERIAL PRIMARY KEY,
            status TEXT NOT NULL,
            task_type TEXT,
            payload JSONB,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            completed_at TIMESTAMPTZ
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
        'CREATE INDEX IF NOT EXISTS idx_%s_tasks_type ON %I.tasks(task_type)',
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

COMMIT;
