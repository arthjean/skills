#!/usr/bin/env bash
# posthog-persons.sh - list / get / delete persons + bulk delete + property edits.
# Replaces MCP tools: persons-list, persons-retrieve, persons-bulk-delete, persons-property-set,
#                     persons-property-delete, persons-cohorts-retrieve, persons-values-retrieve.
#
# Usage:
#   ./posthog-persons.sh ls            [limit]                                  [project_id]
#   ./posthog-persons.sh get           <person_id>                              [project_id]
#   ./posthog-persons.sh find          <email-or-distinct_id-substring>         [project_id]
#   ./posthog-persons.sh by-email      <email>                                  [project_id]
#   ./posthog-persons.sh by-distinct   <distinct_id>                            [project_id]
#   ./posthog-persons.sh activity      <person_id>                              [project_id]
#   ./posthog-persons.sh cohorts       <person_id>                              [project_id]
#   ./posthog-persons.sh values        <prop_key> [limit]                       [project_id]
#   ./posthog-persons.sh set-prop      <person_id> <key> <value>                [project_id]
#   ./posthog-persons.sh del-prop      <person_id> <key>                        [project_id]
#   ./posthog-persons.sh rm            <person_id>                              [project_id]
#   ./posthog-persons.sh bulk-rm       <ids-csv>                                [project_id]
#   ./posthog-persons.sh bulk-rm-distinct <distinct_ids-csv>                    [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|find|by-email|by-distinct|activity|cohorts|values|set-prop|del-prop|rm|bulk-rm|bulk-rm-distinct} [args...]"

action="$1"; shift

case "$action" in
  ls)
    limit="${1:-100}"; pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/?limit=${limit}" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <person_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/$1/" | pretty
    ;;

  find)
    [[ $# -ge 1 ]] || err "usage: $0 find <substring> [project_id]"
    q=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/?search=${q}" | pretty
    ;;

  by-email)
    [[ $# -ge 1 ]] || err "usage: $0 by-email <email> [project_id]"
    em=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/?email=${em}" | pretty
    ;;

  by-distinct)
    [[ $# -ge 1 ]] || err "usage: $0 by-distinct <distinct_id> [project_id]"
    did=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/?distinct_id=${did}" | pretty
    ;;

  activity)
    [[ $# -ge 1 ]] || err "usage: $0 activity <person_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/$1/activity/" | pretty
    ;;

  cohorts)
    [[ $# -ge 1 ]] || err "usage: $0 cohorts <person_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/persons/$1/cohorts/" | pretty
    ;;

  values)
    [[ $# -ge 1 ]] || err "usage: $0 values <prop_key> [limit] [project_id]"
    key=$(urlencode "$1"); limit="${2:-50}"; pid=$(resolve_project_id "${3:-}")
    posthog_api GET "/api/projects/$pid/persons/values/?key=${key}&limit=${limit}" | pretty
    ;;

  set-prop)
    [[ $# -ge 3 ]] || err "usage: $0 set-prop <person_id> <key> <value> [project_id]"
    id="$1"; key="$2"; val="$3"; pid=$(resolve_project_id "${4:-}")
    body=$(jq -nc --arg k "$key" --arg v "$val" '{key:$k, value:$v}')
    posthog_api POST "/api/projects/$pid/persons/$id/update_property/" "$body" | pretty
    ;;

  del-prop)
    [[ $# -ge 2 ]] || err "usage: $0 del-prop <person_id> <key> [project_id]"
    id="$1"; key="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg k "$key" '{$key: $k} | with_entries(select(.value!=null)) | {key: $k}')
    posthog_api POST "/api/projects/$pid/persons/$id/delete_property/" "$body" | pretty
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <person_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/persons/$1/" | pretty
    ;;

  bulk-rm)
    [[ $# -ge 1 ]] || err "usage: $0 bulk-rm <ids-csv> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg ids "$1" '{ids: ($ids | split(",") | map(tonumber? // .))}')
    posthog_api POST "/api/projects/$pid/persons/bulk_delete/" "$body" | pretty
    ;;

  bulk-rm-distinct)
    [[ $# -ge 1 ]] || err "usage: $0 bulk-rm-distinct <distinct_ids-csv> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --arg ids "$1" '{distinct_ids: ($ids | split(","))}')
    posthog_api POST "/api/projects/$pid/persons/bulk_delete/" "$body" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
