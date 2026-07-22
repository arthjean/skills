#!/usr/bin/env bash
# posthog-flags.sh - feature flags CRUD + sub-actions.
# Replaces MCP tools: feature-flag-get-all, feature-flag-get-definition, create-feature-flag,
#                     update-feature-flag, delete-feature-flag, feature-flags-status-retrieve,
#                     feature-flags-dependent-flags-retrieve, feature-flags-evaluation-reasons-retrieve,
#                     feature-flags-activity-retrieve, feature-flags-copy-flags-create,
#                     feature-flags-user-blast-radius-create.
#
# Usage:
#   ./posthog-flags.sh ls                                                       [project_id]
#   ./posthog-flags.sh get        <flag_id|key>                                 [project_id]
#   ./posthog-flags.sh by-key     <key>                                         [project_id]
#   ./posthog-flags.sh create     <key> <name> [active] [rollout%]              [project_id]
#   ./posthog-flags.sh create-json <body-json>                                  [project_id]
#   ./posthog-flags.sh update     <flag_id> <patch-json>                        [project_id]
#   ./posthog-flags.sh enable     <flag_id>                                     [project_id]
#   ./posthog-flags.sh disable    <flag_id>                                     [project_id]
#   ./posthog-flags.sh rollout    <flag_id> <percent>                           [project_id]
#   ./posthog-flags.sh rm         <flag_id>                                     [project_id]
#   ./posthog-flags.sh status     <flag_id>                                     [project_id]
#   ./posthog-flags.sh dependents <flag_id>                                     [project_id]
#   ./posthog-flags.sh activity   <flag_id>                                     [project_id]
#   ./posthog-flags.sh copy       <flag_id> <target_project_ids-csv>            [project_id]
#   ./posthog-flags.sh blast      <flag_id> <condition-set>                     [project_id]
#   ./posthog-flags.sh local-eval                                               [project_id]
#   ./posthog-flags.sh schedule   <flag_id> <date> <change-json>                [project_id]
#   ./posthog-flags.sh schedules  <flag_id>                                     [project_id]
#   ./posthog-flags.sh schedule-rm <flag_id> <change_id>                        [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|by-key|create|create-json|update|enable|disable|rollout|rm|status|dependents|activity|copy|blast|local-eval|schedule|schedules|schedule-rm} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/?limit=200" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <flag_id|key> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/$1/" | pretty
    ;;

  by-key)
    [[ $# -ge 1 ]] || err "usage: $0 by-key <key> [project_id]"
    k=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/?key=${k}" | pretty
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <key> <name> [active] [rollout%] [project_id]"
    key="$1"; name="$2"; active="${3:-true}"; rollout="${4:-100}"; pid=$(resolve_project_id "${5:-}")
    body=$(jq -nc --arg k "$key" --arg n "$name" --argjson a "$active" --argjson r "$rollout" '
      {key:$k, name:$n, active:$a,
       filters:{groups:[{properties:[],rollout_percentage:$r}], multivariate:null, payloads:{}}}')
    posthog_api POST "/api/projects/$pid/feature_flags/" "$body" | pretty
    ;;

  create-json)
    [[ $# -ge 1 ]] || err "usage: $0 create-json <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/feature_flags/" "$1" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <flag_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/feature_flags/$1/" "$2" | pretty
    ;;

  enable)
    [[ $# -ge 1 ]] || err "usage: $0 enable <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/projects/$pid/feature_flags/$1/" '{"active":true}' | pretty
    ;;

  disable)
    [[ $# -ge 1 ]] || err "usage: $0 disable <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/projects/$pid/feature_flags/$1/" '{"active":false}' | pretty
    ;;

  rollout)
    [[ $# -ge 2 ]] || err "usage: $0 rollout <flag_id> <percent> [project_id]"
    fid="$1"; pct="$2"; pid=$(resolve_project_id "${3:-}")
    current=$(posthog_api GET "/api/projects/$pid/feature_flags/$fid/")
    body=$(printf '%s' "$current" | jq --argjson p "$pct" \
      '{filters: (.filters // {} | .groups |= ((. // [{properties:[],rollout_percentage:0}]) | map(.rollout_percentage = $p)))}')
    posthog_api PATCH "/api/projects/$pid/feature_flags/$fid/" "$body" | pretty
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/feature_flags/$1/" | pretty
    ;;

  status)
    [[ $# -ge 1 ]] || err "usage: $0 status <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/$1/status/" | pretty
    ;;

  dependents)
    [[ $# -ge 1 ]] || err "usage: $0 dependents <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/$1/dependent_flags/" | pretty
    ;;

  activity)
    [[ $# -ge 1 ]] || err "usage: $0 activity <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/$1/activity/" | pretty
    ;;

  copy)
    [[ $# -ge 2 ]] || err "usage: $0 copy <flag_id> <target_project_ids-csv> [project_id]"
    fid="$1"; targets="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --argjson id "$fid" --arg t "$targets" \
      '{feature_flag_key:"", from_project:'"$pid"', target_project_ids: ($t | split(",") | map(tonumber))}')
    posthog_api POST "/api/projects/$pid/feature_flags/copy_flags/" "$body" | pretty
    ;;

  blast)
    [[ $# -ge 2 ]] || err "usage: $0 blast <flag_id> <condition-set-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api POST "/api/projects/$pid/feature_flags/$1/user_blast_radius/" "$2" | pretty
    ;;

  local-eval)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/feature_flags/local_evaluation/" | pretty
    ;;

  schedule)
    [[ $# -ge 3 ]] || err "usage: $0 schedule <flag_id> <iso-date> <change-json> [project_id]"
    fid="$1"; date="$2"; change="$3"; pid=$(resolve_project_id "${4:-}")
    body=$(jq -nc --arg d "$date" --argjson c "$change" --argjson f "$fid" \
      '{record_id:$f, model_name:"FeatureFlag", scheduled_at:$d, payload:$c}')
    posthog_api POST "/api/projects/$pid/scheduled_changes/" "$body" | pretty
    ;;

  schedules)
    [[ $# -ge 1 ]] || err "usage: $0 schedules <flag_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/scheduled_changes/?model_name=FeatureFlag&record_id=$1" | pretty
    ;;

  schedule-rm)
    [[ $# -ge 2 ]] || err "usage: $0 schedule-rm <flag_id> <change_id> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api DELETE "/api/projects/$pid/scheduled_changes/$2/" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
