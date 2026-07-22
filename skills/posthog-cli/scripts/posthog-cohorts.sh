#!/usr/bin/env bash
# posthog-cohorts.sh - full cohort CRUD + add/remove persons (static cohorts).
# Replaces MCP tools: cohorts-list, cohorts-retrieve, cohorts-create, cohorts-partial-update,
#                     cohorts-add-persons-to-static-cohort-partial-update,
#                     cohorts-rm-person-from-static-cohort-partial-update.
#
# Usage:
#   ./posthog-cohorts.sh ls                                                  [project_id]
#   ./posthog-cohorts.sh get        <cohort_id>                              [project_id]
#   ./posthog-cohorts.sh create     <name> <filters-json> [is_static]        [project_id]
#   ./posthog-cohorts.sh create-static <name>                                [project_id]
#   ./posthog-cohorts.sh update     <cohort_id> <patch-json>                 [project_id]
#   ./posthog-cohorts.sh rm         <cohort_id>                              [project_id]
#   ./posthog-cohorts.sh persons    <cohort_id>                              [project_id]
#   ./posthog-cohorts.sh add        <cohort_id> <distinct_ids-csv>           [project_id]
#   ./posthog-cohorts.sh remove     <cohort_id> <distinct_ids-csv>           [project_id]
#   ./posthog-cohorts.sh duplicate  <cohort_id>                              [project_id]
#   ./posthog-cohorts.sh activity   <cohort_id>                              [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|create-static|update|rm|persons|add|remove|duplicate|activity} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/cohorts/?limit=200" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <cohort_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/cohorts/$1/" | pretty
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <name> <filters-json> [is_static] [project_id]"
    name="$1"; filters="$2"; is_static="${3:-false}"; pid=$(resolve_project_id "${4:-}")
    body=$(jq -nc --arg n "$name" --argjson f "$filters" --argjson s "$is_static" \
      '{name:$n, filters:$f, is_static:$s}')
    posthog_api POST "/api/projects/$pid/cohorts/" "$body" | pretty
    ;;

  create-static)
    [[ $# -ge 1 ]] || err "usage: $0 create-static <name> [project_id]"
    name="$1"; pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg n "$name" '{name:$n, is_static:true, filters:{properties:{type:"AND",values:[]}}}')
    posthog_api POST "/api/projects/$pid/cohorts/" "$body" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <cohort_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/cohorts/$1/" "$2" | pretty
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <cohort_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/cohorts/$1/" | pretty
    ;;

  persons)
    [[ $# -ge 1 ]] || err "usage: $0 persons <cohort_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/cohorts/$1/persons/" | pretty
    ;;

  add)
    [[ $# -ge 2 ]] || err "usage: $0 add <cohort_id> <distinct_ids-csv> [project_id]"
    cid="$1"; ids="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg ids "$ids" '{distinct_ids: ($ids | split(","))}')
    posthog_api POST "/api/projects/$pid/cohorts/$cid/persons/" "$body" | pretty
    ;;

  remove)
    [[ $# -ge 2 ]] || err "usage: $0 remove <cohort_id> <distinct_ids-csv> [project_id]"
    cid="$1"; ids="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg ids "$ids" '{distinct_ids: ($ids | split(","))}')
    posthog_api DELETE "/api/projects/$pid/cohorts/$cid/persons/" "$body" | pretty
    ;;

  duplicate)
    [[ $# -ge 1 ]] || err "usage: $0 duplicate <cohort_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/cohorts/$1/duplicate_as_static_cohort/" | pretty
    ;;

  activity)
    [[ $# -ge 1 ]] || err "usage: $0 activity <cohort_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/cohorts/$1/activity/" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
