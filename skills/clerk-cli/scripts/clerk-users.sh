#!/usr/bin/env bash
# clerk-users.sh - full user CRUD via Backend API.
# Replaces @clerk/agent-toolkit MCP tools: getUserList, getUser, getUserCount, createUser, updateUser, deleteUser.
# Adds: search by email/phone, ban/unban, lock/unlock, metadata merge, sessions list.
#
# Usage:
#   ./clerk-users.sh ls       [limit] [offset] [order_by]
#   ./clerk-users.sh count
#   ./clerk-users.sh get      <user_id>
#   ./clerk-users.sh find     <email-or-phone-substring>
#   ./clerk-users.sh create   <email> <password-source> [first_name] [last_name]
#   ./clerk-users.sh update   <user_id> <patch-json>
#   ./clerk-users.sh metadata <user_id> {public|private|unsafe} <merge-json>
#   ./clerk-users.sh ban      <user_id>
#   ./clerk-users.sh unban    <user_id>
#   ./clerk-users.sh lock     <user_id>
#   ./clerk-users.sh unlock   <user_id>
#   ./clerk-users.sh sessions <user_id>
#   ./clerk-users.sh orgs     <user_id>
#   ./clerk-users.sh rm       <user_id>

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|count|get|find|create|update|metadata|ban|unban|lock|unlock|sessions|orgs|rm} [args...]"

action="$1"; shift

case "$action" in
  ls)
    limit="${1:-20}"; offset="${2:-0}"; order="${3:--created_at}"
    clerk_api GET "/users?limit=${limit}&offset=${offset}&order_by=$(urlencode "$order")" | jq .
    ;;

  count)
    clerk_api GET "/users/count" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <user_id>"
    clerk_api GET "/users/$1" | jq .
    ;;

  find)
    [[ $# -ge 1 ]] || err "usage: $0 find <email-or-phone-substring>"
    q=$(urlencode "$1")
    clerk_api GET "/users?query=${q}&limit=20" | jq .
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <email> <@env:VARIABLE|-> [first_name] [last_name]"
    email="$1"; password=$(read_secret_arg "$2" "password"); first="${3:-}"; last="${4:-}"
    body=$(jq -nc \
      --arg email "$email" \
      --arg password "$password" \
      --arg first "$first" \
      --arg last "$last" \
      '{email_address: [$email], password: $password}
       + (if $first != "" then {first_name: $first} else {} end)
       + (if $last  != "" then {last_name:  $last}  else {} end)')
    clerk_api POST "/users" "$body" | jq .
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <user_id> <patch-json>"
    clerk_api PATCH "/users/$1" "$2" | jq .
    ;;

  metadata)
    [[ $# -ge 3 ]] || err "usage: $0 metadata <user_id> {public|private|unsafe} <merge-json>"
    uid="$1"; kind="$2"; patch="$3"
    case "$kind" in
      public|private|unsafe) ;;
      *) err "metadata kind must be: public | private | unsafe" ;;
    esac
    # API version 2026-05-12 provides an atomic deep-merge endpoint.
    body=$(jq -nc --argjson m "$patch" --arg k "${kind}_metadata" '{($k): $m}')
    clerk_api PATCH "/users/$uid/metadata" "$body" | jq .
    ;;

  ban)
    [[ $# -ge 1 ]] || err "usage: $0 ban <user_id>"
    clerk_api POST "/users/$1/ban" | jq .
    ;;

  unban)
    [[ $# -ge 1 ]] || err "usage: $0 unban <user_id>"
    clerk_api POST "/users/$1/unban" | jq .
    ;;

  lock)
    [[ $# -ge 1 ]] || err "usage: $0 lock <user_id>"
    clerk_api POST "/users/$1/lock" | jq .
    ;;

  unlock)
    [[ $# -ge 1 ]] || err "usage: $0 unlock <user_id>"
    clerk_api POST "/users/$1/unlock" | jq .
    ;;

  sessions)
    [[ $# -ge 1 ]] || err "usage: $0 sessions <user_id>"
    clerk_api GET "/sessions?user_id=$1" | jq .
    ;;

  orgs)
    [[ $# -ge 1 ]] || err "usage: $0 orgs <user_id>"
    clerk_api GET "/users/$1/organization_memberships" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <user_id>"
    clerk_api DELETE "/users/$1" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
