#!/usr/bin/env bash
# resend-broadcasts.sh - marketing broadcasts (one-to-many emails to a segment/audience).
# MCP tools replaced: create_broadcast, send_broadcast, list_broadcasts, get_broadcast,
#                     update_broadcast, remove_broadcast
#
# Subcommands:
#   create   Create a broadcast (draft)
#              Flags: --from <addr> --subject <s> --html <str|@file> [--text <str|@file>]
#                     [--name <internal>] [--reply-to <addr>] [--segment <segment_id>]
#                     [--template <template_id>]
#   ls       List broadcasts
#   get      Get a single broadcast     get <id>
#   update   Update a draft broadcast   update <id> [--subject ...] [--html @file] [--text @file] [--name ...]
#   send     Send a broadcast           send <id> [--scheduled-at <iso8601|natural>]
#   rm       Delete a broadcast         rm <id>

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|send|rm} [args...]"
action="$1"; shift

_read_inline_or_file() {
  local v="${1:-}"
  [[ -z "$v" ]] && return 0
  if [[ "$v" == @* ]]; then
    local f="${v:1}"; [[ -f "$f" ]] || err "file not found: $f"
    cat "$f"
  else
    printf '%s' "$v"
  fi
}

case "$action" in
  create)
    from="${RESEND_FROM:-}"; subject=""; html=""; text=""; name=""; reply_to=""; segment=""; template=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --from)     from="$2"; shift 2 ;;
        --subject)  subject="$2"; shift 2 ;;
        --html)     html=$(_read_inline_or_file "$2"); shift 2 ;;
        --text)     text=$(_read_inline_or_file "$2"); shift 2 ;;
        --name)     name="$2"; shift 2 ;;
        --reply-to) reply_to="$2"; shift 2 ;;
        --segment)  segment="$2"; shift 2 ;;
        --template) template="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$from" ]] || err "missing --from (or export RESEND_FROM=...)"
    [[ -n "$subject" ]] || err "missing --subject"
    [[ -n "$html" || -n "$text" || -n "$template" ]] || err "must supply --html or --text or --template"
    body=$(jq -nc --arg f "$from" --arg s "$subject" '{from:$f, subject:$s}')
    [[ -n "$html" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$html"     '. + {html: $v}')
    [[ -n "$text" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$text"     '. + {text: $v}')
    [[ -n "$name" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$name"     '. + {name: $v}')
    [[ -n "$reply_to" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$reply_to" '. + {reply_to: $v}')
    [[ -n "$segment" ]]  && body=$(printf '%s' "$body" | jq -c --arg v "$segment"  '. + {segment_id: $v}')
    [[ -n "$template" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$template" '. + {template_id: $v}')
    resend_api POST "/broadcasts" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/broadcasts" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <broadcast_id>"
    resend_api GET "/broadcasts/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <broadcast_id> [flags...]"
    bid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --subject)  body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {subject: $v}'); shift 2 ;;
        --html)     v=$(_read_inline_or_file "$2"); body=$(printf '%s' "$body" | jq -c --arg v "$v" '. + {html: $v}'); shift 2 ;;
        --text)     v=$(_read_inline_or_file "$2"); body=$(printf '%s' "$body" | jq -c --arg v "$v" '. + {text: $v}'); shift 2 ;;
        --name)     body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {name: $v}'); shift 2 ;;
        --reply-to) body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {reply_to: $v}'); shift 2 ;;
        --from)     body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {from: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/broadcasts/$bid" "$body" | pretty
    ;;

  send)
    [[ $# -ge 1 ]] || err "usage: $0 send <broadcast_id> [--scheduled-at <iso8601|natural>]"
    bid="$1"; shift
    body='{}'; sched=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --scheduled-at) sched="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$sched" ]] && body=$(jq -nc --arg v "$sched" '{scheduled_at: $v}')
    if [[ "$body" == "{}" ]]; then
      resend_api POST "/broadcasts/$bid/send" | pretty
    else
      resend_api POST "/broadcasts/$bid/send" "$body" | pretty
    fi
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <broadcast_id>"
    resend_api DELETE "/broadcasts/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|send|rm)" ;;
esac

