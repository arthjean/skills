#!/usr/bin/env bash

set -euo pipefail

err() {
  printf 'ERROR: %s\n' "$1" >&2
  exit 1
}

require_neon_api_key() {
  [[ -n "${NEON_API_KEY:-}" ]] || err \
    "NEON_API_KEY is not set. Export a scoped Neon API key before running this command."
}

validate_connection_mode() {
  case "$1" in
    direct | pooled)
      ;;
    *)
      err "connection mode must be 'direct' or 'pooled', got: $1"
      ;;
  esac
}

# Let neonctl resolve the nearest .neon file when no explicit project is supplied.
neon_conn() {
  local branch="${1:?branch required}"
  local mode="${2:-direct}"
  local project_id="${3:-${NEON_PROJECT_ID:-}}"
  local database="${4:-}"

  validate_connection_mode "$mode"

  local project_args=()
  local pooled_args=()
  local database_args=()

  [[ -n "$project_id" ]] && project_args=(--project-id "$project_id")
  [[ "$mode" == "pooled" ]] && pooled_args=(--pooled)
  [[ -n "$database" ]] && database_args=(--database-name "$database")

  bunx neonctl@latest cs "$branch" \
    "${project_args[@]}" \
    "${database_args[@]}" \
    "${pooled_args[@]}" \
    --no-color
}

neon_psql_c() {
  local branch="${1:?branch required}"
  local sql="${2:?sql required}"
  local mode="${3:-pooled}"
  local project_id="${4:-}"
  local database="${5:-}"
  local conn

  conn="$(neon_conn "$branch" "$mode" "$project_id" "$database")"
  psql "$conn" -v ON_ERROR_STOP=1 -At -c "$sql"
}

# A direct connection is the safe default because transaction files often contain DDL.
neon_psql_tx() {
  local branch="${1:?branch required}"
  local file="${2:-}"
  local mode="${3:-direct}"
  local project_id="${4:-}"
  local database="${5:-}"
  local conn

  conn="$(neon_conn "$branch" "$mode" "$project_id" "$database")"

  if [[ -n "$file" ]]; then
    [[ -f "$file" ]] || err "file not found: $file"
    psql "$conn" -v ON_ERROR_STOP=1 -1 -f "$file"
  else
    psql "$conn" -v ON_ERROR_STOP=1 -1
  fi
}
