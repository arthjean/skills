#!/usr/bin/env bash
# clerk-invitations.sh - app-level invitation CRUD (not org-scoped).
# Replaces @clerk/agent-toolkit MCP tools: createInvitation, getInvitationList, revokeInvitation.
# Adds: bulk invitation from a file (one email per line), respecting the bulk endpoint.
#
# Usage:
#   ./clerk-invitations.sh ls       [status] [limit] [offset]
#   ./clerk-invitations.sh create   <email> [redirect_url] [public_metadata_json]
#   ./clerk-invitations.sh revoke   <invitation_id>
#   ./clerk-invitations.sh bulk     <emails_file> [redirect_url]
#
# Statuses: pending | accepted | revoked | expired
# Rate limits: POST /invitations is 100/hr; bulk is 25/hr (each request can include many emails).

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 <action> [args...]"
action="$1"; shift

case "$action" in
  ls)
    status="${1:-}"; limit="${2:-20}"; offset="${3:-0}"
    qs="limit=${limit}&offset=${offset}"
    [[ -n "$status" ]] && qs="${qs}&status=$(urlencode "$status")"
    clerk_api GET "/invitations?${qs}" | jq .
    ;;

  create)
    [[ $# -ge 1 ]] || err "usage: $0 create <email> [redirect_url] [public_metadata_json]"
    email="$1"; redir="${2:-}"; meta="${3:-}"
    body=$(jq -nc \
      --arg email "$email" \
      --arg redir "$redir" \
      --arg meta "$meta" \
      '{email_address: $email}
       + (if $redir != "" then {redirect_url: $redir} else {} end)
       + (if $meta  != "" then {public_metadata: ($meta | fromjson)} else {} end)')
    clerk_api POST "/invitations" "$body" | jq .
    ;;

  revoke)
    [[ $# -ge 1 ]] || err "usage: $0 revoke <invitation_id>"
    clerk_api POST "/invitations/$1/revoke" | jq .
    ;;

  bulk)
    [[ $# -ge 1 ]] || err "usage: $0 bulk <emails_file> [redirect_url]"
    file="$1"; redir="${2:-}"
    [[ -f "$file" ]] || err "file not found: $file"
    # Build the array body - one entry per non-empty line.
    if [[ -n "$redir" ]]; then
      body=$(jq -Rn --arg redir "$redir" \
        '[inputs | select(length > 0) | {email_address: ., redirect_url: $redir}]' \
        < "$file")
    else
      body=$(jq -Rn '[inputs | select(length > 0) | {email_address: .}]' < "$file")
    fi
    count=$(echo "$body" | jq 'length')
    [[ "$count" -gt 0 ]] || err "no emails in $file"
    printf '\033[33m!\033[0m sending bulk invitation for %s emails (rate limit: 25 bulk req/hr)\n' "$count" >&2
    clerk_api POST "/invitations/bulk" "$body" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
