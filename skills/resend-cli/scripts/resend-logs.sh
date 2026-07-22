#!/usr/bin/env bash
# resend-logs.sh - API request logs (MCP GAP - not exposed by resend-mcp).
# Useful for debugging "why did that webhook/contact/email call fail in production?"
#
# Subcommands:
#   ls    List recent log entries (paginated)
#           Flags: --limit N  (1-100, default 100)
#                  --method GET|POST|PATCH|DELETE
#                  --status <http_code>
#                  --path <substring>      (filter by request path)
#   get   Get a single log entry            get <log_id>
#
# Note: Log retention is gated by plan tier. Free-plan logs are short-lived;
# on paid tiers, logs are retained per the docs' retention policy.

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get} [args...]"
action="$1"; shift

case "$action" in
  ls|list)
    limit="${RESEND_PAGE_LIMIT:-100}"
    method=""; status=""; pathf=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --limit)  limit="$2"; shift 2 ;;
        --method) method="$2"; shift 2 ;;
        --status) status="$2"; shift 2 ;;
        --path)   pathf="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    qs="limit=$limit"
    [[ -n "$method" ]] && qs="${qs}&method=$(urlencode "$method")"
    [[ -n "$status" ]] && qs="${qs}&status_code=$(urlencode "$status")"
    [[ -n "$pathf" ]]  && qs="${qs}&path=$(urlencode "$pathf")"
    resend_paginate "/logs?$qs"
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <log_id>"
    resend_api GET "/logs/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: ls|get)" ;;
esac

