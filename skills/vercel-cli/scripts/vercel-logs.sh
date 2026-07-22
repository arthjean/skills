#!/usr/bin/env bash
# vercel-logs.sh - fetch build logs or runtime logs for a deployment.
# Replaces MCP `get_deployment_build_logs` (build mode) and `get_runtime_logs` (runtime mode).
#
# Usage:
#   ./vercel-logs.sh build   <deployment-url-or-id> [--limit=100]
#   ./vercel-logs.sh runtime <deployment-url-or-id> [--follow] [--since=10m]
#
# Examples:
#   ./vercel-logs.sh build my-app-abc123.vercel.app
#   ./vercel-logs.sh runtime my-app-abc123.vercel.app --follow
#   ./vercel-logs.sh runtime my-app-abc123.vercel.app --since=1h --limit=200

source "$(dirname "$0")/_lib.sh"
require_vercel_token

[[ $# -ge 2 ]] || err "usage: $0 {build|runtime} <deployment-url-or-id> [flags...]"

mode="$1"
target="$2"
shift 2

case "$mode" in
  build)
    # Build logs come from the deployment events stream.
    # Filter by type: stdout/stderr/command/exit produce build progress.
    limit=100
    for a in "$@"; do
      case "$a" in
        --limit=*) limit="${a#--limit=}" ;;
      esac
    done
    path=$(with_team_query "/v3/deployments/$target/events?limit=$limit&direction=backward&follow=0&builds=1")
    vercel_api GET "$path" \
      | jq -r '.[] | "\(.created // .createdAt // 0 | tonumber / 1000 | strftime("%Y-%m-%d %H:%M:%S")) [\(.type // "info")] \(.payload.text // .text // .payload.deploymentId // "")"'
    ;;

  runtime)
    # Runtime logs use vercel CLI for live tailing (it streams over a websocket-like channel).
    follow=0
    extra=()
    for a in "$@"; do
      case "$a" in
        --follow|-f) follow=1 ;;
        *) extra+=("$a") ;;
      esac
    done

    scope_args=()
    if [[ -n "${VERCEL_TEAM_ID:-}" ]]; then
      scope_args+=(--scope "$VERCEL_TEAM_ID")
    fi

    if [[ $follow -eq 1 ]]; then
      bunx vercel@latest logs "$target" \
        --follow \
        "${scope_args[@]}" \
        "${extra[@]}"
    else
      bunx vercel@latest logs "$target" \
        "${scope_args[@]}" \
        "${extra[@]}"
    fi
    ;;

  *)
    err "unknown mode: $mode (expected: build | runtime)"
    ;;
esac
