#!/usr/bin/env bash
# posthog-surveys.sh - surveys CRUD + stats.
# Replaces MCP tools: surveys-get-all, survey-get, survey-create, survey-update, survey-delete,
#                     survey-stats, surveys-global-stats.
#
# Usage:
#   ./posthog-surveys.sh ls                                                  [project_id]
#   ./posthog-surveys.sh get      <survey_id>                                [project_id]
#   ./posthog-surveys.sh create   <body-json>                                [project_id]
#   ./posthog-surveys.sh update   <survey_id> <patch-json>                   [project_id]
#   ./posthog-surveys.sh launch   <survey_id>                                [project_id]
#   ./posthog-surveys.sh stop     <survey_id>                                [project_id]
#   ./posthog-surveys.sh archive  <survey_id>                                [project_id]
#   ./posthog-surveys.sh rm       <survey_id>                                [project_id]
#   ./posthog-surveys.sh stats    <survey_id>                                [project_id]
#   ./posthog-surveys.sh global                                              [project_id]
#   ./posthog-surveys.sh activity                                            [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|launch|stop|archive|rm|stats|global|activity} [args...]"

action="$1"; shift

case "$action" in
  ls)        pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/surveys/?limit=200" | pretty ;;
  get)       [[ $# -ge 1 ]] || err "usage: $0 get <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/surveys/$1/" | pretty ;;
  create)    [[ $# -ge 1 ]] || err "usage: $0 create <body-json> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/surveys/" "$1" | pretty ;;
  update)    [[ $# -ge 2 ]] || err "usage: $0 update <survey_id> <patch-json> [project_id]"
             pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "/api/projects/$pid/surveys/$1/" "$2" | pretty ;;
  launch)    [[ $# -ge 1 ]] || err "usage: $0 launch <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}")
             body=$(jq -nc --arg d "$(date -u +%FT%TZ)" '{start_date:$d}')
             posthog_api PATCH "/api/projects/$pid/surveys/$1/" "$body" | pretty ;;
  stop)      [[ $# -ge 1 ]] || err "usage: $0 stop <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}")
             body=$(jq -nc --arg d "$(date -u +%FT%TZ)" '{end_date:$d}')
             posthog_api PATCH "/api/projects/$pid/surveys/$1/" "$body" | pretty ;;
  archive)   [[ $# -ge 1 ]] || err "usage: $0 archive <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api PATCH "/api/projects/$pid/surveys/$1/" '{"archived":true}' | pretty ;;
  rm)        [[ $# -ge 1 ]] || err "usage: $0 rm <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "/api/projects/$pid/surveys/$1/" | pretty ;;
  stats)     [[ $# -ge 1 ]] || err "usage: $0 stats <survey_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/surveys/$1/stats/" | pretty ;;
  global)    pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/surveys/stats/" | pretty ;;
  activity)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/surveys/activity/" | pretty ;;
  *) err "unknown action: $action" ;;
esac
