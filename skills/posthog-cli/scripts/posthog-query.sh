#!/usr/bin/env bash
# posthog-query.sh - run HogQL (and other query kinds) against a PostHog project.
# Replaces MCP tools: query-run, query-validate, query-logs, query-generate-hogql-from-question,
#                     hogql-schema, get-llm-total-costs-for-project (HogQL recipe).
#
# Usage:
#   ./posthog-query.sh hogql      "<sql>"                              [project_id]
#   ./posthog-query.sh hogql-file <file.sql>                           [project_id]
#   ./posthog-query.sh raw        <json-body>                          [project_id]   # full {query: {...}}
#   ./posthog-query.sh async      "<sql>"                              [project_id]
#   ./posthog-query.sh status     <client_query_id>                    [project_id]
#   ./posthog-query.sh log        <client_query_id>                    [project_id]
#   ./posthog-query.sh cancel     <client_query_id>                    [project_id]
#   ./posthog-query.sh schema                                           [project_id]   # column metadata
#   ./posthog-query.sh validate   "<sql>"                              [project_id]
#   ./posthog-query.sh table      "<sql>"                              [project_id]   # results as TSV
#   ./posthog-query.sh logs       "<filter-json>"                      [project_id]   # logs over HogQL
#
# Environment override: $POSTHOG_QUERY_NAME annotates the query (default "posthog-cli").

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {hogql|hogql-file|raw|async|status|log|cancel|schema|validate|table|logs} [args...]"

action="$1"; shift
name="${POSTHOG_QUERY_NAME:-posthog-cli}"

case "$action" in
  hogql)
    [[ $# -ge 1 ]] || err "usage: $0 hogql <sql> [project_id]"
    sql="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg q "$sql" --arg n "$name" \
      '{query:{kind:"HogQLQuery",query:$q},name:$n}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  hogql-file)
    [[ $# -ge 1 ]] || err "usage: $0 hogql-file <file.sql> [project_id]"
    [[ -f "$1" ]] || err "file not found: $1"
    sql=$(<"$1"); pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg q "$sql" --arg n "$name" \
      '{query:{kind:"HogQLQuery",query:$q},name:$n}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  raw)
    [[ $# -ge 1 ]] || err "usage: $0 raw <json-body> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/query/" "$1" | pretty
    ;;

  async)
    [[ $# -ge 1 ]] || err "usage: $0 async <sql> [project_id]"
    sql="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg q "$sql" --arg n "$name" \
      '{query:{kind:"HogQLQuery",query:$q},name:$n,async:true}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  status)
    [[ $# -ge 1 ]] || err "usage: $0 status <client_query_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/query/$1/" | pretty
    ;;

  log)
    [[ $# -ge 1 ]] || err "usage: $0 log <client_query_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/query/$1/log/" | pretty
    ;;

  cancel)
    [[ $# -ge 1 ]] || err "usage: $0 cancel <client_query_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/query/$1/" | pretty
    ;;

  schema)
    pid=$(resolve_project_id "${1:-}")
    body='{"query":{"kind":"DatabaseSchemaQuery"}}'
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  validate)
    [[ $# -ge 1 ]] || err "usage: $0 validate <sql> [project_id]"
    sql="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg q "$sql" '{query:{kind:"HogQLMetadata",language:"hogQL",query:$q}}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  table)
    [[ $# -ge 1 ]] || err "usage: $0 table <sql> [project_id]"
    sql="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg q "$sql" --arg n "$name" \
      '{query:{kind:"HogQLQuery",query:$q},name:$n}')
    response=$(posthog_api POST "/api/projects/$pid/query/" "$body")
    # Output columns header + tab-separated rows.
    printf '%s' "$response" | jq -r '
      ([.columns | @tsv]),
      (.results[]? | map(if type=="array" or type=="object" then tojson else tostring end) | @tsv)
    '
    ;;

  logs)
    [[ $# -ge 1 ]] || err "usage: $0 logs <filter-json> [project_id]"
    filter="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --argjson f "$filter" '{query:{kind:"LogsQuery"} + $f}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
