#!/usr/bin/env bash
# clerk-allowlist.sh - allowlist + blocklist identifier management.
# Backend API resources the agent-toolkit MCP does not expose.
# Identifiers can be exact emails, phone numbers, or wildcard domains (e.g. "*@example.com").
#
# Usage:
#   ./clerk-allowlist.sh ls    {allow|block}
#   ./clerk-allowlist.sh add   {allow|block} <identifier> [notify=true|false]
#   ./clerk-allowlist.sh rm    {allow|block} <id>
#
# Note: enabling/disabling the allowlist itself is a separate operation:
#   ./clerk-instance.sh restrictions '{"allowlist":true,"blocklist":false}'

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 2 ]] || err "usage: $0 {ls|add|rm} {allow|block} [args...]"
action="$1"; kind="$2"; shift 2

case "$kind" in
  allow) base="/allowlist_identifiers" ;;
  block) base="/blocklist_identifiers" ;;
  *) err "kind must be: allow | block" ;;
esac

case "$action" in
  ls)
    clerk_api GET "$base" | jq .
    ;;

  add)
    [[ $# -ge 1 ]] || err "usage: $0 add $kind <identifier> [notify=true|false]"
    ident="$1"; notify="${2:-false}"
    body=$(jq -nc --arg id "$ident" --argjson n "$notify" '{identifier: $id, notify: $n}')
    clerk_api POST "$base" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm $kind <id>"
    clerk_api DELETE "${base}/$1" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
