#!/usr/bin/env bash
# posthog-dashboards.sh - dashboards CRUD + tile reorder + run.
# Replaces MCP tools: dashboards-get-all, dashboard-get, dashboard-create, dashboard-update,
#                     dashboard-delete, dashboard-reorder-tiles, dashboard-insights-run.
#
# Usage:
#   ./posthog-dashboards.sh ls                                                [project_id]
#   ./posthog-dashboards.sh get      <dashboard_id>                           [project_id]
#   ./posthog-dashboards.sh create   <name> [description] [pinned]           [project_id]
#   ./posthog-dashboards.sh create-json <body-json>                          [project_id]
#   ./posthog-dashboards.sh update   <dashboard_id> <patch-json>             [project_id]
#   ./posthog-dashboards.sh rename   <dashboard_id> <name>                   [project_id]
#   ./posthog-dashboards.sh pin      <dashboard_id>                          [project_id]
#   ./posthog-dashboards.sh unpin    <dashboard_id>                          [project_id]
#   ./posthog-dashboards.sh rm       <dashboard_id>                          [project_id]
#   ./posthog-dashboards.sh refresh  <dashboard_id>                          [project_id]
#   ./posthog-dashboards.sh sharing  <dashboard_id>                          [project_id]
#   ./posthog-dashboards.sh add-tile <dashboard_id> <insight_id>             [project_id]
#   ./posthog-dashboards.sh url      <dashboard_id>                          [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|create-json|update|rename|pin|unpin|rm|refresh|sharing|add-tile|url} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/dashboards/?limit=200" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/dashboards/$1/" | pretty
    ;;

  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <name> [description] [pinned] [project_id]"
    name="$1"; desc="${2:-}"; pinned="${3:-false}"; pid=$(resolve_project_id "${4:-}")
    body=$(jq -nc --arg n "$name" --arg d "$desc" --argjson p "$pinned" \
      '{name:$n} + (if $d != "" then {description:$d} else {} end) + {pinned:$p}')
    posthog_api POST "/api/projects/$pid/dashboards/" "$body" | pretty
    ;;

  create-json)
    [[ $# -ge 1 ]] || err "usage: $0 create-json <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/dashboards/" "$1" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <dashboard_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/dashboards/$1/" "$2" | pretty
    ;;

  rename)
    [[ $# -ge 2 ]] || err "usage: $0 rename <dashboard_id> <name> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg n "$2" '{name:$n}')
    posthog_api PATCH "/api/projects/$pid/dashboards/$1/" "$body" | pretty
    ;;

  pin)
    [[ $# -ge 1 ]] || err "usage: $0 pin <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/projects/$pid/dashboards/$1/" '{"pinned":true}' | pretty
    ;;

  unpin)
    [[ $# -ge 1 ]] || err "usage: $0 unpin <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/projects/$pid/dashboards/$1/" '{"pinned":false}' | pretty
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/dashboards/$1/" | pretty
    ;;

  refresh)
    [[ $# -ge 1 ]] || err "usage: $0 refresh <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/dashboards/$1/?refresh=true" | pretty
    ;;

  sharing)
    [[ $# -ge 1 ]] || err "usage: $0 sharing <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/dashboards/$1/sharing/" | pretty
    ;;

  add-tile)
    [[ $# -ge 2 ]] || err "usage: $0 add-tile <dashboard_id> <insight_id> [project_id]"
    did="$1"; iid="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --argjson iid "$iid" '{insight:$iid}')
    posthog_api POST "/api/projects/$pid/dashboards/$did/insights/" "$body" | pretty
    ;;

  url)
    [[ $# -ge 1 ]] || err "usage: $0 url <dashboard_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    printf '%s/project/%s/dashboard/%s\n' "$POSTHOG_HOST" "$pid" "$1"
    ;;

  *) err "unknown action: $action" ;;
esac
