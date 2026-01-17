BEGIN;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS guid UUID,
    ADD COLUMN IF NOT EXISTS is_demo BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS storage_prefix TEXT;

UPDATE projects SET guid = gen_random_uuid() WHERE guid IS NULL;
UPDATE projects SET storage_prefix = guid::text WHERE storage_prefix IS NULL;
UPDATE projects SET is_demo = TRUE WHERE name = 'demo_wordcount';

CREATE OR REPLACE FUNCTION sanitize_identifier(input TEXT)
RETURNS TEXT AS $$
BEGIN
    RETURN regexp_replace(lower(input), '[^a-z0-9_]+', '_', 'g');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION project_schema_name_legacy(p_id BIGINT, p_name TEXT)
RETURNS TEXT AS $$
DECLARE
    safe_name TEXT;
BEGIN
    safe_name := sanitize_identifier(COALESCE(p_name, 'project'));
    RETURN format('prj_%s_%s', p_id, safe_name);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION project_schema_name(p_id BIGINT, p_name TEXT, p_guid UUID)
RETURNS TEXT AS $$
BEGIN
    RETURN format('prj_%s', replace(p_guid::text, '-', ''));
END;
$$ LANGUAGE plpgsql;

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

CREATE OR REPLACE FUNCTION drop_project_schema(p_id BIGINT, p_name TEXT, p_guid UUID)
RETURNS VOID AS $$
DECLARE
    schema_name TEXT;
BEGIN
    schema_name := project_schema_name(p_id, p_name, p_guid);
    EXECUTE format('DROP SCHEMA IF EXISTS %I CASCADE', schema_name);
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    rec RECORD;
    old_name TEXT;
    new_name TEXT;
BEGIN
    FOR rec IN SELECT id, name, guid FROM projects LOOP
        old_name := project_schema_name_legacy(rec.id, rec.name);
        new_name := project_schema_name(rec.id, rec.name, rec.guid);
        IF old_name <> new_name AND EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = old_name) THEN
            EXECUTE format('ALTER SCHEMA %I RENAME TO %I', old_name, new_name);
        END IF;
    END LOOP;
END $$;

CREATE TABLE IF NOT EXISTS agents (
    id BIGSERIAL PRIMARY KEY,
    agent_uid UUID NOT NULL UNIQUE,
    display_name TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    blocked BOOLEAN NOT NULL DEFAULT FALSE,
    blocked_reason TEXT,
    cpu_limit_percent REAL,
    gpu_limit_percent REAL,
    ram_limit_percent REAL,
    last_seen TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS agent_hardware (
    agent_id BIGINT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    hardware JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS agent_metrics (
    id BIGSERIAL PRIMARY KEY,
    agent_id BIGINT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cpu_load REAL,
    ram_used_mb REAL,
    ram_total_mb REAL,
    gpu_load REAL,
    gpu_mem_used_mb REAL,
    net_rx_bytes BIGINT,
    net_tx_bytes BIGINT,
    disk_read_bytes BIGINT,
    disk_write_bytes BIGINT
);

CREATE INDEX IF NOT EXISTS idx_agent_metrics_agent_id ON agent_metrics(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_metrics_recorded_at ON agent_metrics(recorded_at);

CREATE TABLE IF NOT EXISTS agent_project_preferences (
    agent_id BIGINT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    allowed_task_types TEXT[] NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (agent_id, project_id)
);

CREATE TABLE IF NOT EXISTS project_task_types (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL,
    script_object_key TEXT NOT NULL,
    script_sha256 TEXT NOT NULL,
    script_version TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, task_type)
);

COMMIT;
