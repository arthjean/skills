#!/usr/bin/env bash
# resend-received.sh - inbound (received) emails.
# MCP tools replaced: list_received_emails, get_received_email, list_received_attachments, download_received_attachment
#
# Subcommands:
#   ls            List received emails (paginated)   --limit N
#   get           Get a single received email      get <id>
#   attachments   List attachments on a received   attachments <email_id>
#   attachment    Download a received attachment   attachment <email_id> <attachment_id> [--out <path>]

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|attachments|attachment} [args...]"
action="$1"; shift

case "$action" in
  ls|list)
    limit="${RESEND_PAGE_LIMIT:-100}"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --limit) limit="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    resend_paginate "/received-emails?limit=$limit"
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <received_email_id>"
    resend_api GET "/received-emails/$1" | pretty
    ;;

  attachments)
    [[ $# -ge 1 ]] || err "usage: $0 attachments <received_email_id>"
    resend_api GET "/received-emails/$1/attachments" | pretty
    ;;

  attachment)
    [[ $# -ge 2 ]] || err "usage: $0 attachment <received_email_id> <attachment_id> [--out <path>]"
    eid="$1"; aid="$2"; shift 2
    out=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --out) out="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    resp=$(resend_api GET "/received-emails/$eid/attachments/$aid")
    if [[ -n "$out" ]]; then
      [[ ! -e "$out" ]] || err "refusing to overwrite existing path: $out"
      # If response contains base64 content, decode to file; otherwise dump JSON.
      content=$(printf '%s' "$resp" | jq -r '.content // .data // ""' 2>/dev/null || echo "")
      if [[ -n "$content" && "$content" != "null" ]]; then
        printf '%s' "$content" | base64 -d > "$out" 2>/dev/null || printf '%s' "$content" > "$out"
        printf 'OK: attachment saved to %s\n' "$out"
      else
        printf '%s' "$resp" | pretty
      fi
    else
      printf '%s' "$resp" | pretty
    fi
    ;;

  *) err "unknown action: $action  (try: ls|get|attachments|attachment)" ;;
esac
