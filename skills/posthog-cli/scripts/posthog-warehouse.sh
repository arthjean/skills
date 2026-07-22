#!/usr/bin/env bash
# posthog-warehouse.sh - data warehouse: sources, schemas, saved queries (views).
# Replaces MCP tools: external-data-sources-*, external-data-schemas-*, view-*,
#                     data-warehouse-data-health-issues-retrieve.
#
# Usage:
#   ./posthog-warehouse.sh sources                                          [project_id]
#   ./posthog-warehouse.sh source-get      <source_id>                      [project_id]
#   ./posthog-warehouse.sh source-create   <body-json>                      [project_id]
#   ./posthog-warehouse.sh source-update   <source_id> <patch-json>         [project_id]
#   ./posthog-warehouse.sh source-rm       <source_id>                      [project_id]
#   ./posthog-warehouse.sh source-reload   <source_id>                      [project_id]
#   ./posthog-warehouse.sh source-jobs     <source_id>                      [project_id]
#   ./posthog-warehouse.sh source-schemas                                   [project_id]
#   ./posthog-warehouse.sh schema-get      <schema_id>                      [project_id]
#   ./posthog-warehouse.sh schema-update   <schema_id> <patch-json>         [project_id]
#   ./posthog-warehouse.sh schema-cancel   <schema_id>                      [project_id]
#   ./posthog-warehouse.sh schema-resync   <schema_id>                      [project_id]
#   ./posthog-warehouse.sh schema-reload   <schema_id>                      [project_id]
#   ./posthog-warehouse.sh tables                                           [project_id]
#   ./posthog-warehouse.sh views                                            [project_id]
#   ./posthog-warehouse.sh view-get        <view_id>                        [project_id]
#   ./posthog-warehouse.sh view-create     <body-json>                      [project_id]
#   ./posthog-warehouse.sh view-update     <view_id> <patch-json>           [project_id]
#   ./posthog-warehouse.sh view-rm         <view_id>                        [project_id]
#   ./posthog-warehouse.sh view-run        <view_id>                        [project_id]
#   ./posthog-warehouse.sh view-materialize <view_id>                       [project_id]
#   ./posthog-warehouse.sh view-unmaterialize <view_id>                     [project_id]
#   ./posthog-warehouse.sh health                                           [project_id]

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {sources|source-get|source-create|source-update|source-rm|source-reload|source-jobs|source-schemas|schema-get|schema-update|schema-cancel|schema-resync|schema-reload|tables|views|view-get|view-create|view-update|view-rm|view-run|view-materialize|view-unmaterialize|health} [args...]"

action="$1"; shift

case "$action" in
  sources)              pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/external_data_sources/" | pretty ;;
  source-get)           [[ $# -ge 1 ]] || err "usage: $0 source-get <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/external_data_sources/$1/" | pretty ;;
  source-create)        [[ $# -ge 1 ]] || err "usage: $0 source-create <body-json> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/external_data_sources/" "$1" | pretty ;;
  source-update)        [[ $# -ge 2 ]] || err "usage: $0 source-update <id> <patch-json> [project_id]"
                        pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "/api/projects/$pid/external_data_sources/$1/" "$2" | pretty ;;
  source-rm)            [[ $# -ge 1 ]] || err "usage: $0 source-rm <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "/api/projects/$pid/external_data_sources/$1/" | pretty ;;
  source-reload)        [[ $# -ge 1 ]] || err "usage: $0 source-reload <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/external_data_sources/$1/reload/" | pretty ;;
  source-jobs)          [[ $# -ge 1 ]] || err "usage: $0 source-jobs <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/external_data_sources/$1/jobs/" | pretty ;;
  source-schemas)       pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/external_data_schemas/" | pretty ;;
  schema-get)           [[ $# -ge 1 ]] || err "usage: $0 schema-get <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/external_data_schemas/$1/" | pretty ;;
  schema-update)        [[ $# -ge 2 ]] || err "usage: $0 schema-update <id> <patch-json> [project_id]"
                        pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "/api/projects/$pid/external_data_schemas/$1/" "$2" | pretty ;;
  schema-cancel)        [[ $# -ge 1 ]] || err "usage: $0 schema-cancel <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/external_data_schemas/$1/cancel/" | pretty ;;
  schema-resync)        [[ $# -ge 1 ]] || err "usage: $0 schema-resync <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/external_data_schemas/$1/resync/" | pretty ;;
  schema-reload)        [[ $# -ge 1 ]] || err "usage: $0 schema-reload <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/external_data_schemas/$1/reload/" | pretty ;;
  tables)               pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/warehouse_tables/" | pretty ;;
  views)                pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/warehouse_saved_queries/" | pretty ;;
  view-get)             [[ $# -ge 1 ]] || err "usage: $0 view-get <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api GET "/api/projects/$pid/warehouse_saved_queries/$1/" | pretty ;;
  view-create)          [[ $# -ge 1 ]] || err "usage: $0 view-create <body-json> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/warehouse_saved_queries/" "$1" | pretty ;;
  view-update)          [[ $# -ge 2 ]] || err "usage: $0 view-update <id> <patch-json> [project_id]"
                        pid=$(resolve_project_id "${3:-}"); posthog_api PATCH "/api/projects/$pid/warehouse_saved_queries/$1/" "$2" | pretty ;;
  view-rm)              [[ $# -ge 1 ]] || err "usage: $0 view-rm <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api DELETE "/api/projects/$pid/warehouse_saved_queries/$1/" | pretty ;;
  view-run)             [[ $# -ge 1 ]] || err "usage: $0 view-run <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/warehouse_saved_queries/$1/run/" | pretty ;;
  view-materialize)     [[ $# -ge 1 ]] || err "usage: $0 view-materialize <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/warehouse_saved_queries/$1/materialize/" | pretty ;;
  view-unmaterialize)   [[ $# -ge 1 ]] || err "usage: $0 view-unmaterialize <id> [project_id]"
                        pid=$(resolve_project_id "${2:-}"); posthog_api POST "/api/projects/$pid/warehouse_saved_queries/$1/unmaterialize/" | pretty ;;
  health)               pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/data_warehouse_data_health_issues/" | pretty ;;
  *) err "unknown action: $action" ;;
esac
