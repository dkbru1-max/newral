BEGIN;

DO $$
DECLARE
    rec RECORD;
    schema_name TEXT;
BEGIN
    FOR rec IN SELECT id, name, guid FROM projects LOOP
        schema_name := project_schema_name(rec.id, rec.name, rec.guid);
        EXECUTE format(
            'ALTER TABLE IF EXISTS %I.tasks ADD COLUMN IF NOT EXISTS task_type TEXT',
            schema_name
        );
    END LOOP;
END $$;

COMMIT;
