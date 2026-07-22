#!/usr/bin/env bash

source "$(dirname "$0")/_lib.sh"
require_neon_api_key

[[ $# -ge 2 ]] || err \
  "usage: $0 <branch> <table> [schema] [database] [project_id]"

branch="$1"
table="$2"
schema="${3:-public}"
database="${4:-}"
project_id="${5:-}"

conn="$(neon_conn "$branch" pooled "$project_id" "$database")"

# format(%I) quotes identifiers safely before resolving their catalog OID.
psql "$conn" -v ON_ERROR_STOP=1 -At \
  -v "schema=$schema" \
  -v "table=$table" <<'SQL'
SELECT json_build_object(
  'schema', :'schema',
  'table', :'table',
  'exists', to_regclass(format('%I.%I', :'schema', :'table')) IS NOT NULL,
  'columns', (
    SELECT COALESCE(
      json_agg(
        json_build_object(
          'name', column_name,
          'type', data_type,
          'nullable', is_nullable = 'YES',
          'default', column_default,
          'position', ordinal_position
        )
        ORDER BY ordinal_position
      ),
      '[]'::json
    )
    FROM information_schema.columns
    WHERE table_schema = :'schema'
      AND table_name = :'table'
  ),
  'primary_key', (
    SELECT COALESCE(
      json_agg(a.attname ORDER BY array_position(con.conkey, a.attnum)),
      '[]'::json
    )
    FROM pg_constraint con
    JOIN pg_attribute a
      ON a.attrelid = con.conrelid
     AND a.attnum = ANY(con.conkey)
    WHERE con.contype = 'p'
      AND con.conrelid = to_regclass(format('%I.%I', :'schema', :'table'))
  ),
  'foreign_keys', (
    SELECT COALESCE(
      json_agg(
        json_build_object(
          'name', con.conname,
          'columns', (
            SELECT array_agg(a.attname ORDER BY array_position(con.conkey, a.attnum))
            FROM pg_attribute a
            WHERE a.attrelid = con.conrelid
              AND a.attnum = ANY(con.conkey)
          ),
          'ref_table', con.confrelid::regclass::text,
          'ref_columns', (
            SELECT array_agg(a.attname ORDER BY array_position(con.confkey, a.attnum))
            FROM pg_attribute a
            WHERE a.attrelid = con.confrelid
              AND a.attnum = ANY(con.confkey)
          )
        )
      ),
      '[]'::json
    )
    FROM pg_constraint con
    WHERE con.contype = 'f'
      AND con.conrelid = to_regclass(format('%I.%I', :'schema', :'table'))
  ),
  'indexes', (
    SELECT COALESCE(
      json_agg(
        json_build_object(
          'name', indexname,
          'definition', indexdef
        )
      ),
      '[]'::json
    )
    FROM pg_indexes
    WHERE schemaname = :'schema'
      AND tablename = :'table'
  )
);
SQL
