#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 1 ]] || err \
  "usage: $0 <branch> [limit] [database] [project_id]"

branch="$1"
limit="${2:-20}"
database="${3:-}"
project_id="${4:-}"

[[ "$limit" =~ ^[1-9][0-9]*$ ]] || err \
  "limit must be a positive integer, got: $limit"

conn="$(neon_conn "$branch" pooled "$project_id" "$database")"

extension_loaded="$(
  psql "$conn" -v ON_ERROR_STOP=1 -At \
    -c "SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements'"
)"

[[ -n "$extension_loaded" ]] || err \
  "pg_stat_statements is not loaded on this branch"

psql "$conn" -v ON_ERROR_STOP=1 -At -v "lim=$limit" <<'SQL'
SELECT COALESCE(json_agg(t), '[]'::json)
FROM (
  SELECT
    query,
    calls,
    rows,
    round(mean_exec_time::numeric, 2) AS mean_ms,
    round(total_exec_time::numeric, 2) AS total_ms,
    round(
      (
        100.0 * total_exec_time
        / NULLIF(SUM(total_exec_time) OVER (), 0)
      )::numeric,
      2
    ) AS pct_total_time
  FROM pg_stat_statements
  WHERE query NOT ILIKE '%pg_stat_statements%'
    AND query NOT ILIKE 'EXPLAIN%'
  ORDER BY mean_exec_time DESC
  LIMIT :lim
) t;
SQL
