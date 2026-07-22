#!/usr/bin/env bash
# vercel-webhooks.sh - webhook CRUD via REST API.
# Use when explicit project ID filtering or stable raw responses are required.
#
# Usage:
#   ./vercel-webhooks.sh list
#   ./vercel-webhooks.sh get    <webhook-id>
#   ./vercel-webhooks.sh create <url> <event,event,...> [project-id,project-id,...]
#   ./vercel-webhooks.sh rm     <webhook-id>
#
# Common events:
#   deployment.created, deployment.succeeded, deployment.error, deployment.canceled,
#   deployment.ready, project.created, project.removed, domain.created,
#   integration-configuration.removed, integration-configuration.permission-upgraded
#
# Examples:
#   ./vercel-webhooks.sh create https://hooks.example.com/vercel "deployment.succeeded,deployment.error"
#   ./vercel-webhooks.sh create https://hooks.example.com/vercel "deployment.error" prj_abc,prj_xyz

source "$(dirname "$0")/_lib.sh"
require_vercel_token

[[ $# -ge 1 ]] || err "usage: $0 {list|get|create|rm} ..."

action="$1"
shift

case "$action" in
  list)
    path=$(with_team_query "/v1/webhooks")
    vercel_api GET "$path" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <webhook-id>"
    path=$(with_team_query "/v1/webhooks/$1")
    vercel_api GET "$path" | jq .
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <url> <event,event,...> [project-id,project-id,...]"
    url="$1"; events_csv="$2"; projects_csv="${3:-}"

    events_json=$(echo "$events_csv" | tr ',' '\n' | jq -R . | jq -s .)
    if [[ -n "$projects_csv" ]]; then
      projects_json=$(echo "$projects_csv" | tr ',' '\n' | jq -R . | jq -s .)
      body=$(jq -nc \
        --arg url "$url" \
        --argjson events "$events_json" \
        --argjson projects "$projects_json" \
        '{url: $url, events: $events, projectIds: $projects}')
    else
      body=$(jq -nc \
        --arg url "$url" \
        --argjson events "$events_json" \
        '{url: $url, events: $events}')
    fi

    path=$(with_team_query "/v1/webhooks")
    vercel_api POST "$path" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <webhook-id>"
    path=$(with_team_query "/v1/webhooks/$1")
    vercel_api DELETE "$path" | jq .
    ;;

  *)
    err "unknown action: $action (expected: list | get | create | rm)"
    ;;
esac
