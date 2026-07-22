#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 2 ]] || err \
  "usage: $0 <branch> \"<sql>\" [--safe] [project_id] [database]"

branch="$1"
sql="$2"
shift 2

safe=0
project_id=""
database=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --safe)
      safe=1
      ;;
    *)
      if [[ -z "$project_id" ]]; then
        project_id="$1"
      elif [[ -z "$database" ]]; then
        database="$1"
      else
        err "unexpected argument: $1"
      fi
      ;;
  esac
  shift
done

# Reject statement chaining. Call psql directly for SQL containing literal semicolons.
[[ "$sql" != *";"* ]] || err \
  "EXPLAIN accepts one statement without semicolons"

conn="$(neon_conn "$branch" direct "$project_id" "$database")"

if [[ "$safe" -eq 1 ]]; then
  printf '%s\n' \
    'BEGIN;' \
    "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) $sql;" \
    'ROLLBACK;' \
    | psql "$conn" -v ON_ERROR_STOP=1 -At
else
  psql "$conn" -v ON_ERROR_STOP=1 -At \
    -c "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) $sql"
fi
