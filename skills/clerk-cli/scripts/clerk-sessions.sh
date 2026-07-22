#!/usr/bin/env bash
# clerk-sessions.sh - session inspection, revocation, verification, and JWT minting.
# Backend API tools the @clerk/agent-toolkit MCP does not expose.
#
# Usage:
#   ./clerk-sessions.sh ls       [user_id] [status] [limit] [offset]
#   ./clerk-sessions.sh get      <session_id>
#   ./clerk-sessions.sh revoke   <session_id>
#   ./clerk-sessions.sh verify   <session_id> <token>
#   ./clerk-sessions.sh token    <session_id> <jwt_template_name> [expires_in_seconds]
#   ./clerk-sessions.sh revoke-user-sessions <user_id>
#
# Statuses: active | abandoned | expired | ended | removed | replaced | revoked

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 <action> [args...]"
action="$1"; shift

case "$action" in
  ls)
    user="${1:-}"; status="${2:-}"; limit="${3:-20}"; offset="${4:-0}"
    qs="limit=${limit}&offset=${offset}"
    [[ -n "$user"   ]] && qs="${qs}&user_id=$(urlencode "$user")"
    [[ -n "$status" ]] && qs="${qs}&status=$(urlencode "$status")"
    clerk_api GET "/sessions?${qs}" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <session_id>"
    clerk_api GET "/sessions/$1" | jq .
    ;;

  revoke)
    [[ $# -ge 1 ]] || err "usage: $0 revoke <session_id>"
    clerk_api POST "/sessions/$1/revoke" | jq .
    ;;

  verify)
    [[ $# -ge 2 ]] || err "usage: $0 verify <session_id> <token>"
    body=$(jq -nc --arg t "$2" '{token: $t}')
    clerk_api POST "/sessions/$1/verify" "$body" | jq .
    ;;

  token)
    [[ $# -ge 2 ]] || err "usage: $0 token <session_id> <jwt_template_name> [expires_in_seconds]"
    sid="$1"; tpl="$2"; exp="${3:-}"
    if [[ -n "$exp" ]]; then
      body=$(jq -nc --argjson e "$exp" '{expires_in_seconds: $e}')
      clerk_api POST "/sessions/${sid}/tokens/${tpl}" "$body" | jq .
    else
      clerk_api POST "/sessions/${sid}/tokens/${tpl}" | jq .
    fi
    ;;

  revoke-user-sessions)
    [[ $# -ge 1 ]] || err "usage: $0 revoke-user-sessions <user_id>"
    uid="$1"
    sessions=$(clerk_api GET "/sessions?user_id=${uid}&status=active" | jq -r '.[].id')
    if [[ -z "$sessions" ]]; then
      echo "no active sessions for user $uid"; exit 0
    fi
    for sid in $sessions; do
      printf 'revoking %s ... ' "$sid"
      clerk_api POST "/sessions/${sid}/revoke" >/dev/null && echo "ok"
      sleep 0.05
    done
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
