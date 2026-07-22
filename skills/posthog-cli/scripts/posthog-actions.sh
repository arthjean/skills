#!/usr/bin/env bash
# posthog-actions.sh - actions CRUD + count + people.
# Replaces MCP tools: actions-get-all, action-get, action-create, action-update, action-delete.
#
# Usage:
#   ./posthog-actions.sh ls                                              [project_id]
#   ./posthog-actions.sh get     <action_id>                             [project_id]
#   ./posthog-actions.sh create  <body-json>                             [project_id]
#   ./posthog-actions.sh update  <action_id> <patch-json>                [project_id]
#   ./posthog-actions.sh rm      <action_id>                             [project_id]
#   ./posthog-actions.sh count   <action_id>                             [project_id]
#   ./posthog-actions.sh people  <action_id>                             [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|rm|count|people} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/actions/?limit=200" | pretty ;;
  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <action_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/actions/$1/" | pretty ;;
  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/projects/$pid/actions/" "$1" | pretty ;;
  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <action_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/actions/$1/" "$2" | pretty ;;
  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <action_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/actions/$1/" | pretty ;;
  count)
    [[ $# -ge 1 ]] || err "usage: $0 count <action_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/actions/$1/count/" | pretty ;;
  people)
    [[ $# -ge 1 ]] || err "usage: $0 people <action_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/actions/$1/people/" | pretty ;;
  *) err "unknown action: $action" ;;
esac
