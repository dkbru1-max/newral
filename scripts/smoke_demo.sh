#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost}"
TIMEOUT_SECS="${TIMEOUT_SECS:-120}"
INTERVAL_SECS="${INTERVAL_SECS:-3}"

start_response="$(curl -sS -X POST "${BASE_URL}/api/scheduler/v1/demo/wordcount/start?parts=5")"
echo "Start response: ${start_response}"

start_ts=$(date +%s)
while true; do
  now_ts=$(date +%s)
  elapsed=$((now_ts - start_ts))
  if [[ "${elapsed}" -ge "${TIMEOUT_SECS}" ]]; then
    echo "Timeout waiting for demo result"
    exit 1
  fi

  status_response="$(curl -sS "${BASE_URL}/api/scheduler/v1/demo/wordcount/status")"
  total="$(echo "${status_response}" | sed -n 's/.*"total":[[:space:]]*\\([0-9]*\\).*/\\1/p')"
  completed="$(echo "${status_response}" | sed -n 's/.*"completed":[[:space:]]*\\([0-9]*\\).*/\\1/p')"
  echo "Status: ${status_response}"

  if [[ -n "${total}" && -n "${completed}" && "${total}" -gt 0 && "${completed}" -ge "${total}" ]]; then
    result_response="$(curl -sS "${BASE_URL}/api/scheduler/v1/demo/wordcount/result")"
    echo "Result: ${result_response}"
    exit 0
  fi

  sleep "${INTERVAL_SECS}"
done
