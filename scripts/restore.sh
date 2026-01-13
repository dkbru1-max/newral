#!/usr/bin/env bash
set -euo pipefail

COMPOSE="${COMPOSE:-docker compose}"
BACKUP_DIR="${1:-}"

if [[ -z "${BACKUP_DIR}" ]]; then
  echo "Usage: scripts/restore.sh <backup_dir>"
  exit 1
fi

if [[ ! -d "${BACKUP_DIR}" ]]; then
  echo "Backup directory not found: ${BACKUP_DIR}"
  exit 1
fi

if [[ ! -f "${BACKUP_DIR}/postgres.sql.gz" ]]; then
  echo "Missing Postgres backup: ${BACKUP_DIR}/postgres.sql.gz"
  exit 1
fi

if [[ ! -f "${BACKUP_DIR}/minio_data.tar.gz" ]]; then
  echo "Missing MinIO backup: ${BACKUP_DIR}/minio_data.tar.gz"
  exit 1
fi

# Restore database from compressed dump.
echo "Restoring Postgres..."
gunzip -c "${BACKUP_DIR}/postgres.sql.gz" \
  | ${COMPOSE} exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"'

# Replace MinIO volume contents from backup.
echo "Restoring MinIO data volume..."
${COMPOSE} exec -T minio sh -c 'rm -rf /data/*'
cat "${BACKUP_DIR}/minio_data.tar.gz" \
  | ${COMPOSE} exec -T minio sh -c 'tar -C /data -xzf -'

echo "Restore completed from: ${BACKUP_DIR}"
