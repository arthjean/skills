#!/usr/bin/env bash
# posthog-cdp.sh - Customer Data Platform / Hog Functions / Hog Flows.
# Replaces MCP tools: cdp-functions-list, cdp-functions-create, cdp-functions-retrieve,
#                     cdp-functions-partial-update, cdp-functions-delete, cdp-functions-logs-retrieve,
#                     cdp-functions-metrics-retrieve, cdp-functions-invocations-create,
#                     cdp-function-templates-list, cdp-function-templates-retrieve,
#                     hog-flows-logs-retrieve, hog-flows-metrics-retrieve.
#
# Usage:
#   ./posthog-cdp.sh ls                                                     [project_id]
#   ./posthog-cdp.sh get        <function_id>                               [project_id]
#   ./posthog-cdp.sh create     <body-json>                                 [project_id]
#   ./posthog-cdp.sh update     <function_id> <patch-json>                  [project_id]
#   ./posthog-cdp.sh enable     <function_id>                               [project_id]
#   ./posthog-cdp.sh disable    <function_id>                               [project_id]
#   ./posthog-cdp.sh rm         <function_id>                               [project_id]
#   ./posthog-cdp.sh logs       <function_id>                               [project_id]
#   ./posthog-cdp.sh metrics    <function_id>                               [project_id]
#   ./posthog-cdp.sh invoke     <function_id> <event-json>                  [project_id]
#   ./posthog-cdp.sh templates                                              [project_id]
#   ./posthog-cdp.sh template-get <template_id>                             [project_id]
#   ./posthog-cdp.sh flow-logs    <flow_id>                                 [project_id]
#   ./posthog-cdp.sh flow-metrics <flow_id>                                 [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|enable|disable|rm|logs|metrics|invoke|templates|template-get|flow-logs|flow-metrics} [args...]"

action="$1"; shift

case "$action" in
  ls)            pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/hog_functions/" | pretty ;;
  get)           [[ $# -ge 1 ]] || err "usage: $0 get <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_functions/$1/" | pretty ;;
  create)        [[ $# -ge 1 ]] || err "usage: $0 create <body-json> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/hog_functions/" "$1" | pretty ;;
  update)        [[ $# -ge 2 ]] || err "usage: $0 update <id> <patch-json> [project_id]"
                 pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "/api/projects/$pid/hog_functions/$1/" "$2" | pretty ;;
  enable)        [[ $# -ge 1 ]] || err "usage: $0 enable <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api PATCH "/api/projects/$pid/hog_functions/$1/" '{"enabled":true}' | pretty ;;
  disable)       [[ $# -ge 1 ]] || err "usage: $0 disable <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api PATCH "/api/projects/$pid/hog_functions/$1/" '{"enabled":false}' | pretty ;;
  rm)            [[ $# -ge 1 ]] || err "usage: $0 rm <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "/api/projects/$pid/hog_functions/$1/" | pretty ;;
  logs)          [[ $# -ge 1 ]] || err "usage: $0 logs <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_functions/$1/logs/" | pretty ;;
  metrics)       [[ $# -ge 1 ]] || err "usage: $0 metrics <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_functions/$1/metrics/" | pretty ;;
  invoke)        [[ $# -ge 2 ]] || err "usage: $0 invoke <id> <event-json> [project_id]"
                 pid=$(resolve_project_id "${3:-}"); posthog_api POST "/api/projects/$pid/hog_functions/$1/invocations/" "$2" | pretty ;;
  templates)     pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/hog_function_templates/" | pretty ;;
  template-get)  [[ $# -ge 1 ]] || err "usage: $0 template-get <id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_function_templates/$1/" | pretty ;;
  flow-logs)     [[ $# -ge 1 ]] || err "usage: $0 flow-logs <flow_id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_flows/$1/logs/" | pretty ;;
  flow-metrics)  [[ $# -ge 1 ]] || err "usage: $0 flow-metrics <flow_id> [project_id]"
                 pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/hog_flows/$1/metrics/" | pretty ;;
  *) err "unknown action: $action" ;;
esac
