#!/usr/bin/env bash
# clerk-jwt.sh - JWT template CRUD (Backend API resource the agent-toolkit MCP does not expose).
# JWT templates issue session tokens with custom claims for downstream services
# (Supabase, Hasura, custom backends). See SKILL.md "JWT template workflow" for examples.
#
# Usage:
#   ./clerk-jwt.sh ls
#   ./clerk-jwt.sh get     <id>
#   ./clerk-jwt.sh create  <name> <claims-json> [lifetime_seconds] [allowed_clock_skew]
#   ./clerk-jwt.sh update  <id> <claims-json> [lifetime_seconds]
#   ./clerk-jwt.sh rm      <id>
#
# Claims-json supports Clerk shortcodes like {{user.id}}, {{user.primary_email_address}},
# {{org.id}}, {{org.role}}. See clerk.com/docs/backend-requests/making/jwt-templates.

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|create|update|rm} [args...]"
action="$1"; shift

case "$action" in
  ls)
    clerk_api GET "/jwt_templates" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <id>"
    clerk_api GET "/jwt_templates/$1" | jq .
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <name> <claims-json> [lifetime] [skew]"
    name="$1"; claims="$2"; lifetime="${3:-60}"; skew="${4:-5}"
    body=$(jq -nc \
      --arg name "$name" \
      --argjson claims "$claims" \
      --argjson life "$lifetime" \
      --argjson skew "$skew" \
      '{name: $name, claims: $claims, lifetime: $life, allowed_clock_skew: $skew}')
    clerk_api POST "/jwt_templates" "$body" | jq .
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <id> <claims-json> [lifetime]"
    id="$1"; claims="$2"; lifetime="${3:-}"
    if [[ -n "$lifetime" ]]; then
      body=$(jq -nc --argjson c "$claims" --argjson l "$lifetime" '{claims: $c, lifetime: $l}')
    else
      body=$(jq -nc --argjson c "$claims" '{claims: $c}')
    fi
    clerk_api PATCH "/jwt_templates/$id" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <id>"
    clerk_api DELETE "/jwt_templates/$1" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
