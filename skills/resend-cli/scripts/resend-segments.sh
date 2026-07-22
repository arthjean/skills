#!/usr/bin/env bash
# resend-segments.sh - contact segments (groupings used by broadcasts).
# MCP tools replaced: create_segment, list_segments, get_segment, list_segment_contacts, remove_segment
#
# Subcommands:
#   create   Create a segment
#              Flags: --name <name> [--description <text>] [--filter <@file.json|json>]
#   ls       List all segments
#   get      Get a single segment            get <id>
#   contacts List contacts in a segment      contacts <id>   [--limit N]
#   rm       Delete a segment                rm <id>

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|contacts|rm} [args...]"
action="$1"; shift

case "$action" in
  create)
    name=""; desc=""; filter=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        name="$2"; shift 2 ;;
        --description) desc="$2"; shift 2 ;;
        --filter)
          if [[ "$2" == @* ]]; then
            f="${2:1}"; [[ -f "$f" ]] || err "file not found: $f"
            filter=$(cat "$f")
          else
            filter="$2"
          fi
          shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name"
    body=$(jq -nc --arg n "$name" '{name:$n}')
    [[ -n "$desc" ]]   && body=$(printf '%s' "$body" | jq -c --arg v "$desc" '. + {description: $v}')
    [[ -n "$filter" ]] && body=$(printf '%s' "$body" | jq -c --argjson v "$filter" '. + {filter: $v}')
    resend_api POST "/segments" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/segments" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <segment_id>"
    resend_api GET "/segments/$1" | pretty
    ;;

  contacts)
    [[ $# -ge 1 ]] || err "usage: $0 contacts <segment_id> [--limit N]"
    sid="$1"; shift
    limit="${RESEND_PAGE_LIMIT:-100}"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --limit) limit="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    resend_paginate "/segments/$sid/contacts?limit=$limit"
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <segment_id>"
    resend_api DELETE "/segments/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|contacts|rm)" ;;
esac

