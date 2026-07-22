#!/usr/bin/env bash
# posthog-insights.sh - insights CRUD + run + sharing.
# Replaces MCP tools: insights-list, insight-get, insight-create, insight-update, insight-delete,
#                     insight-query, dashboard-insights-run.
#
# Usage:
#   ./posthog-insights.sh ls                                                     [project_id]
#   ./posthog-insights.sh search   <query>                                       [project_id]
#   ./posthog-insights.sh get      <insight_id>                                  [project_id]
#   ./posthog-insights.sh by-short <short_id>                                    [project_id]
#   ./posthog-insights.sh create   <body-json>                                   [project_id]
#   ./posthog-insights.sh update   <insight_id> <patch-json>                     [project_id]
#   ./posthog-insights.sh rename   <insight_id> <name>                           [project_id]
#   ./posthog-insights.sh rm       <insight_id>                                  [project_id]
#   ./posthog-insights.sh run      <insight_id> [refresh]                        [project_id]   # refresh: blocking|async|force
#   ./posthog-insights.sh sharing  <insight_id>                                  [project_id]
#   ./posthog-insights.sh activity                                                [project_id]
#   ./posthog-insights.sh url      <insight_id>                                  [project_id]   # links to PostHog UI

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|search|get|by-short|create|update|rename|rm|run|sharing|activity|url} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/insights/?limit=200" | pretty
    ;;

  search)
    [[ $# -ge 1 ]] || err "usage: $0 search <query> [project_id]"
    q=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/insights/?search=${q}" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <insight_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/insights/$1/" | pretty
    ;;

  by-short)
    [[ $# -ge 1 ]] || err "usage: $0 by-short <short_id> [project_id]"
    s=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/insights/?short_id=${s}" | pretty
    ;;

  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/insights/" "$1" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <insight_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/insights/$1/" "$2" | pretty
    ;;

  rename)
    [[ $# -ge 2 ]] || err "usage: $0 rename <insight_id> <name> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg n "$2" '{name:$n}')
    posthog_api PATCH "/api/projects/$pid/insights/$1/" "$body" | pretty
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <insight_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/insights/$1/" | pretty
    ;;

  run)
    [[ $# -ge 1 ]] || err "usage: $0 run <insight_id> [refresh] [project_id]"
    refresh="${2:-blocking}"; pid=$(resolve_project_id "${3:-}")
    posthog_api GET "/api/projects/$pid/insights/$1/?refresh=${refresh}" | pretty
    ;;

  sharing)
    [[ $# -ge 1 ]] || err "usage: $0 sharing <insight_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/insights/$1/sharing/" | pretty
    ;;

  activity)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/insights/activity/" | pretty
    ;;

  url)
    [[ $# -ge 1 ]] || err "usage: $0 url <insight_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    short=$(posthog_api GET "/api/projects/$pid/insights/$1/" | jq -r '.short_id')
    printf '%s/project/%s/insights/%s\n' "$POSTHOG_HOST" "$pid" "$short"
    ;;

  *) err "unknown action: $action" ;;
esac
