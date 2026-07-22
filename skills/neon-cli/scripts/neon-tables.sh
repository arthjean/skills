#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 1 ]] || err \
  "usage: $0 <branch> [database] [project_id]"

branch="$1"
database="${2:-}"
project_id="${3:-}"

conn="$(neon_conn "$branch" pooled "$project_id" "$database")"

psql "$conn" -v ON_ERROR_STOP=1 -At <<'SQL'
SELECT COALESCE(json_agg(t), '[]'::json)
FROM (
  SELECT
    tables.schemaname AS schema,
    tables.tablename AS name,
    tables.tableowner AS owner,
    tables.hasindexes AS has_indexes,
    catalog.reltuples::bigint AS row_estimate
  FROM pg_tables AS tables
  LEFT JOIN pg_class AS catalog
    ON catalog.oid = to_regclass(
      format('%I.%I', tables.schemaname, tables.tablename)
    )
  WHERE tables.schemaname NOT IN ('pg_catalog', 'information_schema')
  ORDER BY tables.schemaname, tables.tablename
) t;
SQL
