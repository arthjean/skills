#!/usr/bin/env bash
# posthog-experiments.sh - experiments CRUD + lifecycle (launch/pause/resume/end/archive/ship).
# Replaces MCP tools: experiment-list, experiment-get-all, experiment-create, experiment-get,
#                     experiment-update, experiment-delete, experiment-launch, experiment-pause,
#                     experiment-resume, experiment-end, experiment-archive, experiment-unarchive,
#                     experiment-duplicate, experiment-reset, experiment-ship-variant,
#                     experiment-results-get, experiment-stats, experiment-timeseries-results.
#
# Usage:
#   ./posthog-experiments.sh ls                                          [project_id]
#   ./posthog-experiments.sh get        <experiment_id>                  [project_id]
#   ./posthog-experiments.sh create     <body-json>                      [project_id]
#   ./posthog-experiments.sh update     <experiment_id> <patch-json>     [project_id]
#   ./posthog-experiments.sh rm         <experiment_id>                  [project_id]
#   ./posthog-experiments.sh launch     <experiment_id>                  [project_id]
#   ./posthog-experiments.sh pause      <experiment_id>                  [project_id]
#   ./posthog-experiments.sh resume     <experiment_id>                  [project_id]
#   ./posthog-experiments.sh end        <experiment_id>                  [project_id]
#   ./posthog-experiments.sh archive    <experiment_id>                  [project_id]
#   ./posthog-experiments.sh unarchive  <experiment_id>                  [project_id]
#   ./posthog-experiments.sh duplicate  <experiment_id>                  [project_id]
#   ./posthog-experiments.sh reset      <experiment_id>                  [project_id]
#   ./posthog-experiments.sh ship       <experiment_id> <variant>        [project_id]
#   ./posthog-experiments.sh results    <experiment_id>                  [project_id]
#   ./posthog-experiments.sh stats      <experiment_id>                  [project_id]
#   ./posthog-experiments.sh timeseries <experiment_id>                  [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|rm|launch|pause|resume|end|archive|unarchive|duplicate|reset|ship|results|stats|timeseries} [args...]"

action="$1"; shift

xp_patch() {
  local fid="$1" body="$2" pid="$3"
  posthog_api PATCH "/api/projects/$pid/experiments/$fid/" "$body" | pretty
}

xp_action() {
  local fid="$1" sub="$2" pid="$3"
  posthog_api POST "/api/projects/$pid/experiments/$fid/$sub/" | pretty
}

case "$action" in
  ls)        pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/experiments/?limit=200" | pretty ;;
  get)       [[ $# -ge 1 ]] || err "usage: $0 get <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/experiments/$1/" | pretty ;;
  create)    [[ $# -ge 1 ]] || err "usage: $0 create <body-json> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/experiments/" "$1" | pretty ;;
  update)    [[ $# -ge 2 ]] || err "usage: $0 update <experiment_id> <patch-json> [project_id]"
             pid=$(resolve_project_id "${3:-}"); xp_patch "$1" "$2" "$pid" ;;
  rm)        [[ $# -ge 1 ]] || err "usage: $0 rm <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "/api/projects/$pid/experiments/$1/" | pretty ;;

  launch)    [[ $# -ge 1 ]] || err "usage: $0 launch <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}")
             xp_patch "$1" "$(jq -nc --arg d "$(date -u +%FT%TZ)" '{start_date:$d, archived:false}')" "$pid" ;;
  pause)     [[ $# -ge 1 ]] || err "usage: $0 pause <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); xp_patch "$1" '{"start_date":null}' "$pid" ;;
  resume)    [[ $# -ge 1 ]] || err "usage: $0 resume <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}")
             xp_patch "$1" "$(jq -nc --arg d "$(date -u +%FT%TZ)" '{start_date:$d, end_date:null}')" "$pid" ;;
  end)       [[ $# -ge 1 ]] || err "usage: $0 end <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}")
             xp_patch "$1" "$(jq -nc --arg d "$(date -u +%FT%TZ)" '{end_date:$d}')" "$pid" ;;

  archive)   [[ $# -ge 1 ]] || err "usage: $0 archive <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); xp_patch "$1" '{"archived":true}' "$pid" ;;
  unarchive) [[ $# -ge 1 ]] || err "usage: $0 unarchive <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); xp_patch "$1" '{"archived":false}' "$pid" ;;

  duplicate) [[ $# -ge 1 ]] || err "usage: $0 duplicate <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); xp_action "$1" "duplicate" "$pid" ;;
  reset)     [[ $# -ge 1 ]] || err "usage: $0 reset <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); xp_action "$1" "reset" "$pid" ;;

  ship)      [[ $# -ge 2 ]] || err "usage: $0 ship <experiment_id> <variant_key> [project_id]"
             pid=$(resolve_project_id "${3:-}")
             body=$(jq -nc --arg v "$2" '{variant:$v}')
             posthog_api POST "/api/projects/$pid/experiments/$1/create_exposure_cohort_for_experiment/" "$body" | pretty
             xp_patch "$1" "$(jq -nc --arg v "$2" '{conclusion:"won", winning_variant:$v, end_date:"'"$(date -u +%FT%TZ)"'"}')" "$pid" ;;

  results)   [[ $# -ge 1 ]] || err "usage: $0 results <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/experiments/$1/results/" | pretty ;;
  stats)     [[ $# -ge 1 ]] || err "usage: $0 stats <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/experiments/$1/stats/" | pretty ;;
  timeseries) [[ $# -ge 1 ]] || err "usage: $0 timeseries <experiment_id> [project_id]"
             pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/experiments/$1/timeseries/" | pretty ;;

  *) err "unknown action: $action" ;;
esac
