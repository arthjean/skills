#!/usr/bin/env bash
# vercel-env.sh - full env var CRUD via REST API.
# Replaces the MCP env tools and goes beyond the CLI by exposing types
# (encrypted | plain | sensitive) and bulk multi-target operations.
#
# Usage:
#   ./vercel-env.sh ls     <project-id-or-name>
#   ./vercel-env.sh get    <project-id-or-name> <env-id>
#   ./vercel-env.sh add    <project-id-or-name> <KEY> <VALUE|@env:NAME|-> <target> [target...] [type]
#   ./vercel-env.sh update <project-id-or-name> <env-id> <new-value|@env:NAME|->
#   ./vercel-env.sh rm     <project-id-or-name> <env-id>
#
# Targets: production | preview | development (one or more, space-separated)
# Types:   encrypted (default) | plain | sensitive
#
# Examples:
#   ./vercel-env.sh ls my-app
#   ./vercel-env.sh add my-app DATABASE_URL @env:DATABASE_URL production preview encrypted
#   printf '%s' "$STRIPE_SECRET" | ./vercel-env.sh add my-app STRIPE_SECRET - production sensitive
#   ./vercel-env.sh rm my-app env_xxx

source "$(dirname "$0")/_lib.sh"
require_vercel_token

[[ $# -ge 2 ]] || err "usage: $0 {ls|get|add|update|rm} <project> [args...]"

resolve_value_arg() {
  local value_arg="${1:?value argument required}"
  case "$value_arg" in
    @env:*)
      local value_name="${value_arg#@env:}"
      [[ "$value_name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] \
        || err "invalid environment variable name: $value_name"
      [[ -v "$value_name" ]] || err "environment variable is not set: $value_name"
      printf '%s' "${!value_name}"
      ;;
    -)
      cat
      ;;
    *)
      printf '%s' "$value_arg"
      ;;
  esac
}

action="$1"
project="$2"
shift 2

case "$action" in
  ls)
    path=$(with_team_query "/v9/projects/$project/env")
    vercel_api GET "$path" | jq '.envs // .'
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <project> <env-id>"
    env_id="$1"
    path=$(with_team_query "/v1/projects/$project/env/$env_id")
    vercel_api GET "$path" | jq .
    ;;

  add)
    [[ $# -ge 3 ]] || err "usage: $0 add <project> <KEY> <VALUE> <target> [target...] [type]"
    key="$1"; value_arg="$2"
    shift 2
    value=$(resolve_value_arg "$value_arg")
    # Remaining args: targets (production|preview|development) and optionally type at the end.
    type="encrypted"
    targets=()
    for a in "$@"; do
      case "$a" in
        production|preview|development) targets+=("$a") ;;
        encrypted|plain|sensitive)      type="$a" ;;
        *) err "unknown arg: $a (expected target or type)" ;;
      esac
    done
    [[ ${#targets[@]} -ge 1 ]] || err "at least one target required (production|preview|development)"

    # Build JSON body.
    targets_json=$(printf '%s\n' "${targets[@]}" | jq -R . | jq -s .)
    body=$(jq -nc \
      --arg key "$key" \
      --arg value "$value" \
      --arg type "$type" \
      --argjson targets "$targets_json" \
      '{key: $key, value: $value, type: $type, target: $targets}')

    path=$(with_team_query "/v10/projects/$project/env")
    vercel_api POST "$path" "$body" | jq .
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <project> <env-id> <new-value>"
    env_id="$1"; new_value=$(resolve_value_arg "$2")
    body=$(jq -nc --arg value "$new_value" '{value: $value}')
    path=$(with_team_query "/v9/projects/$project/env/$env_id")
    vercel_api PATCH "$path" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <project> <env-id>"
    env_id="$1"
    path=$(with_team_query "/v9/projects/$project/env/$env_id")
    vercel_api DELETE "$path" | jq .
    ;;

  *)
    err "unknown action: $action (expected: ls | get | add | update | rm)"
    ;;
esac
