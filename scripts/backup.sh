#!/usr/bin/env bash
set -euo pipefail

COMPOSE="${COMPOSE:-docker compose}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKUP_DIR="${ROOT_DIR}/backups/$(date +%Y%m%d_%H%M%S)"

mkdir -p "${BACKUP_DIR}"

# Dump database to a compressed file.
echo "Backing up Postgres..."
${COMPOSE} exec -T postgres sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB"' \
  | gzip > "${BACKUP_DIR}/postgres.sql.gz"

# Export MinIO volume contents as a tarball.
echo "Backing up MinIO data volume..."
${COMPOSE} exec -T minio sh -c 'tar -C /data -czf - .' > "${BACKUP_DIR}/minio_data.tar.gz"

# Capture config files for portability.
echo "Backing up config files..."
for file in "${ROOT_DIR}/docker-compose.yml" "${ROOT_DIR}/.env" "${ROOT_DIR}/config/config.example.yml"; do
  if [[ -f "${file}" ]]; then
    cp "${file}" "${BACKUP_DIR}/"
  else
    echo "Skip missing config: ${file}"
  fi
done

echo "Backup completed: ${BACKUP_DIR}"
