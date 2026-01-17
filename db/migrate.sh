#!/usr/bin/env bash
set -euo pipefail

: "${POSTGRES_USER:?POSTGRES_USER is required}"
: "${POSTGRES_PASSWORD:?POSTGRES_PASSWORD is required}"
: "${POSTGRES_DB:?POSTGRES_DB is required}"

export PGPASSWORD="${POSTGRES_PASSWORD}"

until pg_isready -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" > /dev/null 2>&1; do
  echo "waiting for postgres..."
  sleep 2
done

migration_dir="/migrations"
if [[ ! -d "${migration_dir}" ]]; then
  echo "migration directory not found: ${migration_dir}"
  exit 1
fi

for file in $(ls -1 "${migration_dir}"/*.sql | sort); do
  echo "applying migration: ${file}"
  psql -h "${POSTGRES_HOST:-postgres}" -p "${POSTGRES_PORT:-5432}" -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" -v ON_ERROR_STOP=1 -f "${file}"
done

echo "migrations complete"
