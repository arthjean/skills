#!/usr/bin/env bash
# posthog-errors.sh - error tracking issues + grouping/suppression/assignment rules.
# Replaces MCP tools: error-tracking-issues-list, error-tracking-issues-retrieve,
#                     error-tracking-issues-partial-update, error-tracking-issues-merge-create,
#                     error-tracking-issues-split-create, error-tracking-grouping-rules-list,
#                     error-tracking-grouping-rules-create, error-tracking-suppression-rules-list,
#                     error-tracking-suppression-rules-create, error-tracking-assignment-rules-list,
#                     error-tracking-assignment-rules-create, query-error-tracking-issues.
#
# Usage:
#   ./posthog-errors.sh ls                                                       [project_id]
#   ./posthog-errors.sh get        <issue_id>                                    [project_id]
#   ./posthog-errors.sh update     <issue_id> <patch-json>                       [project_id]
#   ./posthog-errors.sh resolve    <issue_id>                                    [project_id]
#   ./posthog-errors.sh ignore     <issue_id>                                    [project_id]
#   ./posthog-errors.sh assign     <issue_id> <user_id>                          [project_id]
#   ./posthog-errors.sh merge      <primary_id> <ids-csv>                        [project_id]
#   ./posthog-errors.sh split      <issue_id> <fingerprints-csv>                 [project_id]
#   ./posthog-errors.sh grouping-ls                                              [project_id]
#   ./posthog-errors.sh grouping-add <body-json>                                 [project_id]
#   ./posthog-errors.sh suppress-ls                                              [project_id]
#   ./posthog-errors.sh suppress-add <body-json>                                 [project_id]
#   ./posthog-errors.sh assign-ls                                                [project_id]
#   ./posthog-errors.sh assign-add <body-json>                                   [project_id]
#   ./posthog-errors.sh query      <filter-json>                                 [project_id]   # raw issue HogQL filter

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|update|resolve|ignore|assign|merge|split|grouping-ls|grouping-add|suppress-ls|suppress-add|assign-ls|assign-add|query} [args...]"

action="$1"; shift

case "$action" in
  ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/environments/$pid/error_tracking/issues/?limit=100" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <issue_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api GET "/api/environments/$pid/error_tracking/issues/$1/" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <issue_id> <patch-json> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    posthog_api PATCH "/api/environments/$pid/error_tracking/issues/$1/" "$2" | pretty
    ;;

  resolve)
    [[ $# -ge 1 ]] || err "usage: $0 resolve <issue_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/environments/$pid/error_tracking/issues/$1/" '{"status":"resolved"}' | pretty
    ;;

  ignore)
    [[ $# -ge 1 ]] || err "usage: $0 ignore <issue_id> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api PATCH "/api/environments/$pid/error_tracking/issues/$1/" '{"status":"suppressed"}' | pretty
    ;;

  assign)
    [[ $# -ge 2 ]] || err "usage: $0 assign <issue_id> <user_id> [project_id]"
    pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --argjson u "$2" '{assignee:{type:"user", id:$u}}')
    posthog_api PATCH "/api/environments/$pid/error_tracking/issues/$1/" "$body" | pretty
    ;;

  merge)
    [[ $# -ge 2 ]] || err "usage: $0 merge <primary_id> <ids-csv> [project_id]"
    primary="$1"; ids="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg ids "$ids" '{ids: ($ids | split(","))}')
    posthog_api POST "/api/environments/$pid/error_tracking/issues/$primary/merge/" "$body" | pretty
    ;;

  split)
    [[ $# -ge 2 ]] || err "usage: $0 split <issue_id> <fingerprints-csv> [project_id]"
    iid="$1"; fps="$2"; pid=$(resolve_project_id "${3:-}")
    body=$(jq -nc --arg fps "$fps" '{fingerprints: ($fps | split(","))}')
    posthog_api POST "/api/environments/$pid/error_tracking/issues/$iid/split/" "$body" | pretty
    ;;

  grouping-ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/environments/$pid/error_tracking/grouping_rules/" | pretty
    ;;
  grouping-add)
    [[ $# -ge 1 ]] || err "usage: $0 grouping-add <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/environments/$pid/error_tracking/grouping_rules/" "$1" | pretty
    ;;

  suppress-ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/environments/$pid/error_tracking/suppression_rules/" | pretty
    ;;
  suppress-add)
    [[ $# -ge 1 ]] || err "usage: $0 suppress-add <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/environments/$pid/error_tracking/suppression_rules/" "$1" | pretty
    ;;

  assign-ls)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/environments/$pid/error_tracking/assignment_rules/" | pretty
    ;;
  assign-add)
    [[ $# -ge 1 ]] || err "usage: $0 assign-add <body-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    posthog_api POST "/api/environments/$pid/error_tracking/assignment_rules/" "$1" | pretty
    ;;

  query)
    [[ $# -ge 1 ]] || err "usage: $0 query <filter-json> [project_id]"
    pid=$(resolve_project_id "${2:-}")
    body=$(jq -nc --argjson f "$1" '{query:{kind:"ErrorTrackingQuery"} + $f}')
    posthog_api POST "/api/projects/$pid/query/" "$body" | pretty
    ;;

  *) err "unknown action: $action" ;;
esac
