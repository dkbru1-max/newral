#!/usr/bin/env bash
set -euo pipefail

REGISTRY_HOST="${REGISTRY_HOST:-localhost:5000}"

images=(
  "library/postgres:17.7-bookworm"
  "library/redis:7.4.7-bookworm"
  "library/alpine:3.20"
  "apache/kafka:4.1.1"
  "minio/minio:latest"
  "library/nginx:1.27-alpine"
  "library/node:20-bookworm"
  "library/rust:1.88-bookworm"
  "library/debian:bookworm-slim"
  "library/registry:2"
)

for image in "${images[@]}"; do
  src="docker.io/${image}"
  dst="${REGISTRY_HOST}/${image}"
  docker pull "${src}"
  docker tag "${src}" "${dst}"
  docker push "${dst}"
done

echo "Synced images to ${REGISTRY_HOST}"
