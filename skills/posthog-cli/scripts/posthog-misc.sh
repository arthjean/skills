#!/usr/bin/env bash
# posthog-misc.sh - alerts, subscriptions, comments, integrations, scheduled-changes, early-access,
# proxy, activity log, change requests, approval policies, inbox, SDK doctor, web analytics digest,
# usage metrics, debug.
# Replaces remaining MCP tools not covered by the focused resource scripts.
#
# Subcommand groups:
#   alerts         {ls|get|create|update|rm|simulate}
#   subs           {ls|get|create|update|rm|test|deliveries [sub_id]}
#   comments       {ls|get|count|thread <comment_id>}
#   integrations   {ls|get|rm|channels}
#   scheduled      {ls|get|create|update|rm}
#   early-access   {ls|get|create|update|rm}
#   activity       {project|advanced [filters-json]|filters}
#   change-req     {ls|get}
#   approval       {ls|get}
#   inbox          {reports-ls|reports-get <id>|sources-ls|sources-get <id>|tickets-ls|tickets-get <id>|tickets-update <id> <patch>}
#   sdk-doctor     get
#   web-digest     get
#   usage          {ls|get|create|update|rm}
#   proxy          {ls|get|create|rm|retry}
#   sql-vars       {create|update|rm}
#   debug-mcp      {ui-apps}
#   logs           {count|sparkline|attrs|attr-values|count-ranges} <body-json>

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {alerts|subs|comments|integrations|scheduled|early-access|activity|change-req|approval|inbox|sdk-doctor|web-digest|usage|proxy|sql-vars|debug-mcp|logs} <subcommand> [args...]"

resource="$1"; shift
[[ $# -ge 1 ]] || err "missing subcommand for $resource"

case "$resource" in
  alerts)
    sub="$1"; shift
    case "$sub" in
      ls)       pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/alerts/" | pretty ;;
      get)      [[ $# -ge 1 ]] || err "alerts get <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api GET  "/api/projects/$pid/alerts/$1/" | pretty ;;
      create)   [[ $# -ge 1 ]] || err "alerts create <body-json>"; pid=$(resolve_project_id "${2:-}")
                posthog_api POST "/api/projects/$pid/alerts/" "$1" | pretty ;;
      update)   [[ $# -ge 2 ]] || err "alerts update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
                posthog_api PATCH "/api/projects/$pid/alerts/$1/" "$2" | pretty ;;
      rm)       [[ $# -ge 1 ]] || err "alerts rm <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api DELETE "/api/projects/$pid/alerts/$1/" | pretty ;;
      simulate) [[ $# -ge 1 ]] || err "alerts simulate <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api POST "/api/projects/$pid/alerts/$1/simulate/" | pretty ;;
      *) err "unknown alerts subcommand: $sub" ;;
    esac ;;

  subs)
    sub="$1"; shift
    case "$sub" in
      ls)         pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/subscriptions/" | pretty ;;
      get)        [[ $# -ge 1 ]] || err "subs get <id>"; pid=$(resolve_project_id "${2:-}")
                  posthog_api GET  "/api/projects/$pid/subscriptions/$1/" | pretty ;;
      create)     [[ $# -ge 1 ]] || err "subs create <body-json>"; pid=$(resolve_project_id "${2:-}")
                  posthog_api POST "/api/projects/$pid/subscriptions/" "$1" | pretty ;;
      update)     [[ $# -ge 2 ]] || err "subs update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
                  posthog_api PATCH "/api/projects/$pid/subscriptions/$1/" "$2" | pretty ;;
      rm)         [[ $# -ge 1 ]] || err "subs rm <id>"; pid=$(resolve_project_id "${2:-}")
                  posthog_api DELETE "/api/projects/$pid/subscriptions/$1/" | pretty ;;
      test)       [[ $# -ge 1 ]] || err "subs test <id>"; pid=$(resolve_project_id "${2:-}")
                  posthog_api POST "/api/projects/$pid/subscriptions/$1/test_delivery/" | pretty ;;
      deliveries) pid=$(resolve_project_id "${2:-}")
                  if [[ $# -ge 1 ]]; then
                    posthog_api GET "/api/projects/$pid/subscriptions/$1/deliveries/" | pretty
                  else
                    posthog_api GET "/api/projects/$pid/subscription_deliveries/" | pretty
                  fi ;;
      *) err "unknown subs subcommand: $sub" ;;
    esac ;;

  comments)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/comments/" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "comments get <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET "/api/projects/$pid/comments/$1/" | pretty ;;
      count)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/comments/count/" | pretty ;;
      thread) [[ $# -ge 1 ]] || err "comments thread <comment_id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET "/api/projects/$pid/comments/$1/thread/" | pretty ;;
      *) err "unknown comments subcommand: $sub" ;;
    esac ;;

  integrations)
    sub="$1"; shift
    case "$sub" in
      ls)       pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/integrations/" | pretty ;;
      get)      [[ $# -ge 1 ]] || err "integrations get <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api GET "/api/projects/$pid/integrations/$1/" | pretty ;;
      rm)       [[ $# -ge 1 ]] || err "integrations rm <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api DELETE "/api/projects/$pid/integrations/$1/" | pretty ;;
      channels) [[ $# -ge 1 ]] || err "integrations channels <id>"; pid=$(resolve_project_id "${2:-}")
                posthog_api GET "/api/projects/$pid/integrations/$1/channels/" | pretty ;;
      *) err "unknown integrations subcommand: $sub" ;;
    esac ;;

  scheduled)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/scheduled_changes/" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "scheduled get <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET  "/api/projects/$pid/scheduled_changes/$1/" | pretty ;;
      create) [[ $# -ge 1 ]] || err "scheduled create <body-json>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/scheduled_changes/" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "scheduled update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
              posthog_api PATCH "/api/projects/$pid/scheduled_changes/$1/" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "scheduled rm <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api DELETE "/api/projects/$pid/scheduled_changes/$1/" | pretty ;;
      *) err "unknown scheduled subcommand: $sub" ;;
    esac ;;

  early-access)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/early_access_features/" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "early-access get <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET  "/api/projects/$pid/early_access_features/$1/" | pretty ;;
      create) [[ $# -ge 1 ]] || err "early-access create <body-json>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/early_access_features/" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "early-access update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
              posthog_api PATCH "/api/projects/$pid/early_access_features/$1/" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "early-access rm <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api DELETE "/api/projects/$pid/early_access_features/$1/" | pretty ;;
      *) err "unknown early-access subcommand: $sub" ;;
    esac ;;

  activity)
    sub="$1"; shift
    case "$sub" in
      project) pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/activity_log/" | pretty ;;
      advanced) pid=$(resolve_project_id "${2:-}")
                if [[ $# -ge 1 ]]; then
                  posthog_api POST "/api/projects/$pid/advanced_activity_logs/list/" "$1" | pretty
                else
                  posthog_api GET  "/api/projects/$pid/advanced_activity_logs/" | pretty
                fi ;;
      filters) pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/advanced_activity_logs/filters/" | pretty ;;
      *) err "unknown activity subcommand: $sub" ;;
    esac ;;

  change-req)
    sub="$1"; shift
    case "$sub" in
      ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/change_requests/" | pretty ;;
      get) [[ $# -ge 1 ]] || err "change-req get <id>"; pid=$(resolve_project_id "${2:-}")
           posthog_api GET "/api/projects/$pid/change_requests/$1/" | pretty ;;
      *) err "unknown change-req subcommand: $sub" ;;
    esac ;;

  approval)
    sub="$1"; shift
    case "$sub" in
      ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/approval_policies/" | pretty ;;
      get) [[ $# -ge 1 ]] || err "approval get <id>"; pid=$(resolve_project_id "${2:-}")
           posthog_api GET "/api/projects/$pid/approval_policies/$1/" | pretty ;;
      *) err "unknown approval subcommand: $sub" ;;
    esac ;;

  inbox)
    sub="$1"; shift
    case "$sub" in
      reports-ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/inbox_reports/" | pretty ;;
      reports-get) [[ $# -ge 1 ]] || err "inbox reports-get <id>"; pid=$(resolve_project_id "${2:-}")
                   posthog_api GET "/api/projects/$pid/inbox_reports/$1/" | pretty ;;
      sources-ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/inbox_source_configs/" | pretty ;;
      sources-get) [[ $# -ge 1 ]] || err "inbox sources-get <id>"; pid=$(resolve_project_id "${2:-}")
                   posthog_api GET "/api/projects/$pid/inbox_source_configs/$1/" | pretty ;;
      tickets-ls)  pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/conversations/tickets/" | pretty ;;
      tickets-get) [[ $# -ge 1 ]] || err "inbox tickets-get <id>"; pid=$(resolve_project_id "${2:-}")
                   posthog_api GET "/api/projects/$pid/conversations/tickets/$1/" | pretty ;;
      tickets-update) [[ $# -ge 2 ]] || err "inbox tickets-update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
                      posthog_api PATCH "/api/projects/$pid/conversations/tickets/$1/" "$2" | pretty ;;
      *) err "unknown inbox subcommand: $sub" ;;
    esac ;;

  sdk-doctor)
    sub="$1"; shift
    case "$sub" in
      get) pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/sdk_doctor/" | pretty ;;
      *) err "unknown sdk-doctor subcommand: $sub" ;;
    esac ;;

  web-digest)
    sub="$1"; shift
    case "$sub" in
      get) pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/web_analytics/weekly_digest/" | pretty ;;
      *) err "unknown web-digest subcommand: $sub" ;;
    esac ;;

  usage)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/usage_metrics/" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "usage get <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET  "/api/projects/$pid/usage_metrics/$1/" | pretty ;;
      create) [[ $# -ge 1 ]] || err "usage create <body-json>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/usage_metrics/" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "usage update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
              posthog_api PATCH "/api/projects/$pid/usage_metrics/$1/" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "usage rm <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api DELETE "/api/projects/$pid/usage_metrics/$1/" | pretty ;;
      *) err "unknown usage subcommand: $sub" ;;
    esac ;;

  proxy)
    sub="$1"; shift
    case "$sub" in
      ls)     pid=$(resolve_project_id "${1:-}"); posthog_api GET  "/api/projects/$pid/proxy_records/" | pretty ;;
      get)    [[ $# -ge 1 ]] || err "proxy get <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api GET  "/api/projects/$pid/proxy_records/$1/" | pretty ;;
      create) [[ $# -ge 1 ]] || err "proxy create <body-json>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/proxy_records/" "$1" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "proxy rm <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api DELETE "/api/projects/$pid/proxy_records/$1/" | pretty ;;
      retry)  [[ $# -ge 1 ]] || err "proxy retry <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/proxy_records/$1/retry/" | pretty ;;
      *) err "unknown proxy subcommand: $sub" ;;
    esac ;;

  sql-vars)
    sub="$1"; shift
    case "$sub" in
      create) [[ $# -ge 1 ]] || err "sql-vars create <body-json>"; pid=$(resolve_project_id "${2:-}")
              posthog_api POST "/api/projects/$pid/insight_variables/" "$1" | pretty ;;
      update) [[ $# -ge 2 ]] || err "sql-vars update <id> <patch-json>"; pid=$(resolve_project_id "${3:-}")
              posthog_api PATCH "/api/projects/$pid/insight_variables/$1/" "$2" | pretty ;;
      rm)     [[ $# -ge 1 ]] || err "sql-vars rm <id>"; pid=$(resolve_project_id "${2:-}")
              posthog_api DELETE "/api/projects/$pid/insight_variables/$1/" | pretty ;;
      *) err "unknown sql-vars subcommand: $sub" ;;
    esac ;;

  debug-mcp)
    sub="$1"; shift
    case "$sub" in
      ui-apps) pid=$(resolve_project_id "${1:-}"); posthog_api GET "/api/projects/$pid/debug/mcp_ui_apps/" | pretty ;;
      *) err "unknown debug-mcp subcommand: $sub" ;;
    esac ;;

  logs)
    sub="$1"; shift
    [[ $# -ge 1 ]] || err "logs $sub <body-json> [project_id]"
    body="$1"; pid=$(resolve_project_id "${2:-}")
    case "$sub" in
      count)        posthog_api POST "/api/projects/$pid/logs/count/" "$body" | pretty ;;
      sparkline)    posthog_api POST "/api/projects/$pid/logs/sparkline/" "$body" | pretty ;;
      attrs)        posthog_api POST "/api/projects/$pid/logs/attributes/" "$body" | pretty ;;
      attr-values)  posthog_api POST "/api/projects/$pid/logs/attribute_values/" "$body" | pretty ;;
      count-ranges) posthog_api POST "/api/projects/$pid/logs/count_ranges/" "$body" | pretty ;;
      *) err "unknown logs subcommand: $sub" ;;
    esac ;;

  *) err "unknown resource: $resource" ;;
esac
