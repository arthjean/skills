#!/usr/bin/env bash
# posthog-events.sh - read raw events + manage event/property definitions.
# Replaces MCP tools: event-definitions-list, event-definition-update, properties-list,
#                     entity-search (events portion).
# Note: ingestion (capturing events) is not in scope - use the public /capture/ endpoint
#       with a project API key (phc_*), not a personal key.
#
# Usage:
#   ./posthog-events.sh ls                                                 [project_id]
#   ./posthog-events.sh recent  <event_name> [limit]                       [project_id]
#   ./posthog-events.sh search  <query>                                    [project_id]   # via /events?search=
#   ./posthog-events.sh defs    [search] [type]                            [project_id]   # event_definitions
#   ./posthog-events.sh def-get <id>                                       [project_id]
#   ./posthog-events.sh def-rename <id> <new_name>                         [project_id]
#   ./posthog-events.sh def-update <id> <patch-json>                       [project_id]
#   ./posthog-events.sh props   [search] [type] [group_type_index]         [project_id]   # property_definitions
#   ./posthog-events.sh prop-get <id>                                      [project_id]
#   ./posthog-events.sh prop-update <id> <patch-json>                      [project_id]
#   ./posthog-events.sh values  <prop_key> [event] [limit]                 [project_id]   # distinct values

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|recent|search|defs|def-get|def-rename|def-update|props|prop-get|prop-update|values} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/events/?limit=100" | pretty
    ;;

  recent)
    [[ $# -ge 1 ]] || err "usage: $0 recent <event_name> [limit] [project_id]"
    name=$(urlencode "$1"); limit="${2:-50}"; pid=$(resolve_project_id "${3:-}")
    posthog_api GET "/api/projects/$pid/events/?event=${name}&limit=${limit}" | pretty
    ;;

  search)
    [[ $# -ge 1 ]] || err "usage: $0 search <query> [project_id]"
    q=$(urlencode "$1"); pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/events/?properties=%5B%5D&search=${q}" | pretty
    ;;

  defs)
    search="${1:-}"; etype="${2:-}"; pid=$(resolve_project_id "${3:-}")
    qs="limit=200"
    [[ -n "$search" ]] && qs="${qs}&search=$(urlencode "$search")"
    [[ -n "$etype" ]] && qs="${qs}&event_type=$(urlencode "$etype")"
    posthog_api GET "/api/projects/$pid/event_definitions/?${qs}" | pretty
    ;;

  def-get)
    [[ $# -ge 1 ]] || err "usage: $0 def-get <id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/event_definitions/$1/" | pretty
    ;;

  def-rename)
    [[ $# -ge 2 ]] || err "usage: $0 def-rename <id> <new_name> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg n "$2" '{name:$n}')
    posthog_api PATCH "/api/projects/$pid/event_definitions/$1/" "$body" | pretty
    ;;

  def-update)
    [[ $# -ge 2 ]] || err "usage: $0 def-update <id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/event_definitions/$1/" "$2" | pretty
    ;;

  props)
    search="${1:-}"; ptype="${2:-}"; gindex="${3:-}"; pid=$(resolve_project_id "${4:-}")
    qs="limit=200"
    [[ -n "$search" ]] && qs="${qs}&search=$(urlencode "$search")"
    [[ -n "$ptype"  ]] && qs="${qs}&type=$(urlencode "$ptype")"
    [[ -n "$gindex" ]] && qs="${qs}&group_type_index=${gindex}"
    posthog_api GET "/api/projects/$pid/property_definitions/?${qs}" | pretty
    ;;

  prop-get)
    [[ $# -ge 1 ]] || err "usage: $0 prop-get <id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/property_definitions/$1/" | pretty
    ;;

  prop-update)
    [[ $# -ge 2 ]] || err "usage: $0 prop-update <id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/property_definitions/$1/" "$2" | pretty
    ;;

  values)
    [[ $# -ge 1 ]] || err "usage: $0 values <prop_key> [event] [limit] [project_id]"
    key=$(urlencode "$1"); event="${2:-}"; limit="${3:-50}"; pid=$(resolve_project_id "${4:-}")
    qs="key=${key}&limit=${limit}"
    [[ -n "$event" ]] && qs="${qs}&event_name=$(urlencode "$event")"
    posthog_api GET "/api/projects/$pid/events/values/?${qs}" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
