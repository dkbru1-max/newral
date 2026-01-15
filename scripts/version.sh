#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   ./scripts/version.sh major|minor|patch
#   ./scripts/version.sh set 1.2.3
#   ./scripts/version.sh current
# Optional:
#   ./scripts/version.sh minor --tag

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_FILE="${ROOT_DIR}/VERSION"

read_version() {
  if [[ ! -f "${VERSION_FILE}" ]]; then
    echo "VERSION file not found at ${VERSION_FILE}" >&2
    exit 1
  fi
  tr -d ' \t\r\n' < "${VERSION_FILE}"
}

write_version() {
  local version="$1"
  if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Invalid version: ${version}" >&2
    exit 1
  fi
  printf '%s\n' "${version}" > "${VERSION_FILE}"
}

bump_version() {
  local current="$1"
  local part="$2"
  IFS='.' read -r major minor patch <<< "${current}"
  case "${part}" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
    *) echo "Unknown bump: ${part}" >&2; exit 1 ;;
  esac
  echo "${major}.${minor}.${patch}"
}

create_tag=false
if [[ "${*:-}" == *"--tag"* ]]; then
  create_tag=true
fi

case "${1:-}" in
  current)
    read_version
    exit 0
    ;;
  set)
    if [[ -z "${2:-}" ]]; then
      echo "Usage: $0 set X.Y.Z" >&2
      exit 1
    fi
    next_version="${2}"
    ;;
  major|minor|patch)
    current="$(read_version)"
    next_version="$(bump_version "${current}" "${1}")"
    ;;
  *)
    echo "Usage: $0 major|minor|patch|set X.Y.Z|current [--tag]" >&2
    exit 1
    ;;
esac

write_version "${next_version}"
echo "${next_version}"

if ${create_tag}; then
  git -C "${ROOT_DIR}" tag "v${next_version}"
fi
