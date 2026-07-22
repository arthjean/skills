#!/usr/bin/env bash
# posthog-annotations.sh - annotations CRUD.
# Replaces MCP tools: annotations-list, annotation-retrieve, annotation-create,
#                     annotations-partial-update, annotation-delete.
#
# Usage:
#   ./posthog-annotations.sh ls                                              [project_id]
#   ./posthog-annotations.sh get     <annotation_id>                         [project_id]
#   ./posthog-annotations.sh create  <content> [iso-date] [scope]            [project_id]
#                                                  scope: project (default) | dashboard_item | organization
#   ./posthog-annotations.sh update  <annotation_id> <patch-json>            [project_id]
#   ./posthog-annotations.sh rm      <annotation_id>                         [project_id]
#   ./posthog-annotations.sh release <content> [version]                     [project_id]   # convenience: "Released v$version"

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|rm|release} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/annotations/?limit=200" | pretty ;;
  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <annotation_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/projects/$pid/annotations/$1/" | pretty ;;
  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <content> [iso-date] [scope] [project_id]"
    content="$1"; date="${2:-$(date -u +%FT%TZ)}"; scope="${3:-project}"; pid=$(resolve_project_id "${4:-}")
    body=$(jq -nc --arg c "$content" --arg d "$date" --arg s "$scope" \
      '{content:$c, date_marker:$d, scope:$s}')
    posthog_api POST "/api/projects/$pid/annotations/" "$body" | pretty ;;
  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <annotation_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/projects/$pid/annotations/$1/" "$2" | pretty ;;
  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <annotation_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api DELETE "/api/projects/$pid/annotations/$1/" | pretty ;;
  release)
    [[ $# -ge 1 ]] || err "usage: $0 release <content> [version] [project_id]"
    content="$1"
    if [[ $# -ge 2 ]]; then
      content="Released v$2: $1"
    fi
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg c "$content" --arg d "$(date -u +%FT%TZ)" '{content:$c, date_marker:$d, scope:"project"}')
    posthog_api POST "/api/projects/$pid/annotations/" "$body" | pretty ;;
  *) err "unknown action: $action" ;;
esac
