#!/usr/bin/env bash
# posthog-notebooks.sh - notebooks CRUD.
# Replaces MCP tools: notebooks-list, notebooks-retrieve, notebooks-create,
#                     notebooks-partial-update, notebooks-destroy.
#
# Usage:
#   ./posthog-notebooks.sh ls                                              [project_id]
#   ./posthog-notebooks.sh get     <short_id>                              [project_id]
#   ./posthog-notebooks.sh create  <title> [content-json]                  [project_id]
#   ./posthog-notebooks.sh update  <short_id> <patch-json>                 [project_id]
#   ./posthog-notebooks.sh rename  <short_id> <new_title>                  [project_id]
#   ./posthog-notebooks.sh rm      <short_id>                              [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|rename|rm} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/notebooks/?limit=200" | pretty ;;
  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <short_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/notebooks/$1/" | pretty ;;
  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <title> [content-json] [project_id]"
    title="$1"; content="${2:-{\"type\":\"doc\",\"content\":[]}}"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg t "$title" --argjson c "$content" '{title:$t, content:$c}')
    posthog_api POST "/api/projects/$pid/notebooks/" "$body" | pretty ;;
  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <short_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/notebooks/$1/" "$2" | pretty ;;
  rename)
    [[ $# -ge 2 ]] || err "usage: $0 rename <short_id> <title> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg t "$2" '{title:$t}')
    posthog_api PATCH "/api/projects/$pid/notebooks/$1/" "$body" | pretty ;;
  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <short_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/notebooks/$1/" | pretty ;;
  *) err "unknown action: $action" ;;
esac
