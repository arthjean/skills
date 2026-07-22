#!/usr/bin/env bash
# resend-emails.sh - send and manage emails via the Resend REST API.
# MCP tools replaced: send_email, batch_send_emails, list_emails, get_email, update_email, cancel_email
#
# Subcommands:
#   send         Send a single email
#                  Flags: --from <addr> --to <a,b,c> --subject <s> --html <str|@file>
#                         --text <str|@file> --cc <a,b> --bcc <a,b> --reply-to <a>
#                         --scheduled-at <iso8601|natural>  --idempotency-key <key>
#                         --tag name=value (repeatable)  --attach <path>  --topic <id>
#                         --header K=V (repeatable)
#                  Env defaults: RESEND_FROM
#   batch        Send up to 100 emails in one call (NO attachments, NO scheduled_at)
#                  Usage: resend-emails.sh batch @emails.json
#                         resend-emails.sh batch '[{...},{...}]'
#   ls           List sent emails (paginated, follows has_more)
#                  Flags: --limit N (1-100, default 100)
#   get          Get a single sent email by id
#                  Usage: resend-emails.sh get <id>
#   cancel       Cancel a scheduled email
#                  Usage: resend-emails.sh cancel <id>
#   reschedule   Update scheduled_at of a scheduled email (PATCH)
#                  Usage: resend-emails.sh reschedule <id> <scheduled_at>
#   attachments  List attachments on a sent email
#                  Usage: resend-emails.sh attachments <email_id>
#   attachment   Get a single attachment of a sent email
#                  Usage: resend-emails.sh attachment <email_id> <attachment_id>
#
# Examples:
#   resend-emails.sh send --to user@x.com --subject Hi --html '<p>Hi</p>'
#   resend-emails.sh send --from 'A <a@x.com>' --to 'b@x.com,c@x.com' --subject Test \
#                         --html @body.html --tag campaign=launch --idempotency-key signup-42
#   resend-emails.sh batch @batch.json
#   resend-emails.sh ls --limit 50 | jq .

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {send|batch|ls|get|cancel|reschedule|attachments|attachment} [args...]"

action="$1"; shift

# Read a value that may be inline or @file. Returns "" if input is empty.
_read_inline_or_file() {
  local v="${1:-}"
  [[ -z "$v" ]] && return 0
  if [[ "$v" == @* ]]; then
    local f="${v:1}"
    [[ -f "$f" ]] || err "file not found: $f"
    cat "$f"
  else
    printf '%s' "$v"
  fi
}

# Build a JSON array from a comma-separated list.
_csv_to_json_array() {
  local csv="${1:-}"
  [[ -z "$csv" ]] && printf '[]' && return 0
  local IFS=','; read -ra arr <<< "$csv"
  printf '%s' "$(printf '%s\n' "${arr[@]}" | jq -R . | jq -s .)"
}

case "$action" in
  send)
    from="${RESEND_FROM:-}"; to=""; subject=""; html=""; text=""; cc=""; bcc=""; reply_to=""
    scheduled_at=""; idempotency_key=""; topic_id=""
    declare -a tag_args=(); declare -a attach_paths=(); declare -a header_args=()
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --from)             from="$2"; shift 2 ;;
        --to)               to="$2"; shift 2 ;;
        --subject)          subject="$2"; shift 2 ;;
        --html)             html=$(_read_inline_or_file "$2"); shift 2 ;;
        --text)             text=$(_read_inline_or_file "$2"); shift 2 ;;
        --cc)               cc="$2"; shift 2 ;;
        --bcc)              bcc="$2"; shift 2 ;;
        --reply-to)         reply_to="$2"; shift 2 ;;
        --scheduled-at)     scheduled_at="$2"; shift 2 ;;
        --idempotency-key)  idempotency_key="$2"; shift 2 ;;
        --topic)            topic_id="$2"; shift 2 ;;
        --tag)              tag_args+=("$2"); shift 2 ;;
        --attach)           attach_paths+=("$2"); shift 2 ;;
        --header)           header_args+=("$2"); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done

    [[ -n "$from" ]]    || err "missing --from (or export RESEND_FROM='Name <addr@domain>')"
    [[ -n "$to" ]]      || err "missing --to <email[,email...]>"
    [[ -n "$subject" ]] || err "missing --subject"
    [[ -n "$html" || -n "$text" ]] || err "must supply --html OR --text (one is enough)"

    to_arr=$(_csv_to_json_array "$to")
    cc_arr=$(_csv_to_json_array "$cc")
    bcc_arr=$(_csv_to_json_array "$bcc")

    # Build base body via jq for safe escaping.
    body=$(jq -nc \
      --arg from "$from" \
      --argjson to "$to_arr" \
      --arg subject "$subject" \
      '{from: $from, to: $to, subject: $subject}')

    [[ -n "$html" ]]         && body=$(printf '%s' "$body" | jq -c --arg v "$html" '. + {html: $v}')
    [[ -n "$text" ]]         && body=$(printf '%s' "$body" | jq -c --arg v "$text" '. + {text: $v}')
    [[ -n "$cc" ]]           && body=$(printf '%s' "$body" | jq -c --argjson v "$cc_arr" '. + {cc: $v}')
    [[ -n "$bcc" ]]          && body=$(printf '%s' "$body" | jq -c --argjson v "$bcc_arr" '. + {bcc: $v}')
    [[ -n "$reply_to" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$reply_to" '. + {reply_to: $v}')
    [[ -n "$scheduled_at" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$scheduled_at" '. + {scheduled_at: $v}')
    [[ -n "$topic_id" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$topic_id" '. + {topic_id: $v}')

    if [[ ${#tag_args[@]} -gt 0 ]]; then
      tags_json='[]'
      for kv in "${tag_args[@]}"; do
        name="${kv%%=*}"; value="${kv#*=}"
        [[ "$name" == "$value" || -z "$name" ]] && err "--tag must be name=value (got: $kv)"
        tags_json=$(printf '%s' "$tags_json" | jq -c --arg n "$name" --arg v "$value" '. + [{name:$n, value:$v}]')
      done
      body=$(printf '%s' "$body" | jq -c --argjson v "$tags_json" '. + {tags: $v}')
    fi

    if [[ ${#header_args[@]} -gt 0 ]]; then
      headers_json='{}'
      for kv in "${header_args[@]}"; do
        name="${kv%%=*}"; value="${kv#*=}"
        [[ "$name" == "$value" || -z "$name" ]] && err "--header must be Name=Value (got: $kv)"
        headers_json=$(printf '%s' "$headers_json" | jq -c --arg n "$name" --arg v "$value" '. + {($n): $v}')
      done
      body=$(printf '%s' "$body" | jq -c --argjson v "$headers_json" '. + {headers: $v}')
    fi

    if [[ ${#attach_paths[@]} -gt 0 ]]; then
      atts_json='[]'
      for p in "${attach_paths[@]}"; do
        [[ -f "$p" ]] || err "attachment file not found: $p"
        fname=$(basename "$p")
        # Resend accepts base64-encoded content or a URL. For local files, base64 inline.
        b64=$(base64 -w0 "$p" 2>/dev/null || base64 "$p" | tr -d '\n')
        atts_json=$(printf '%s' "$atts_json" | jq -c --arg fn "$fname" --arg c "$b64" '. + [{filename:$fn, content:$c}]')
      done
      body=$(printf '%s' "$body" | jq -c --argjson v "$atts_json" '. + {attachments: $v}')
    fi

    if [[ -n "$idempotency_key" ]]; then
      RESEND_IDEMPOTENCY_KEY="$idempotency_key" resend_api POST "/emails" "$body" | pretty
    else
      resend_api POST "/emails" "$body" | pretty
    fi
    ;;

  batch)
    [[ $# -ge 1 ]] || err "usage: $0 batch <@file.json|json-array>"
    arg="$1"
    if [[ "$arg" == @* ]]; then
      f="${arg:1}"
      [[ -f "$f" ]] || err "file not found: $f"
      body=$(cat "$f")
    else
      body="$arg"
    fi
    # Validate it's a JSON array.
    printf '%s' "$body" | jq -e 'type == "array"' >/dev/null 2>&1 || err "batch body must be a JSON array of email objects"
    n=$(printf '%s' "$body" | jq 'length')
    [[ "$n" -le 100 ]] || err "batch supports at most 100 emails (got $n)"
    # Resend forbids attachments and scheduled_at in batch - warn if present.
    has_att=$(printf '%s' "$body" | jq '[.[] | has("attachments")] | any')
    has_sched=$(printf '%s' "$body" | jq '[.[] | has("scheduled_at")] | any')
    [[ "$has_att" == "true" ]]   && warn "batch ignores 'attachments' on every item - send those individually instead."
    [[ "$has_sched" == "true" ]] && warn "batch ignores 'scheduled_at' on every item - send those individually instead."
    resend_api POST "/emails/batch" "$body" | pretty
    ;;

  ls|list)
    limit="${RESEND_PAGE_LIMIT:-100}"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --limit) limit="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    resend_paginate "/emails?limit=$limit"
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <email_id>"
    resend_api GET "/emails/$1" | pretty
    ;;

  cancel)
    [[ $# -ge 1 ]] || err "usage: $0 cancel <email_id>   # only works for emails with scheduled_at"
    resend_api DELETE "/emails/$1" | pretty
    ;;

  reschedule|update)
    [[ $# -ge 2 ]] || err "usage: $0 reschedule <email_id> <new_scheduled_at>"
    eid="$1"; new_when="$2"
    body=$(jq -nc --arg v "$new_when" '{scheduled_at: $v}')
    resend_api PATCH "/emails/$eid" "$body" | pretty
    ;;

  attachments)
    [[ $# -ge 1 ]] || err "usage: $0 attachments <email_id>"
    resend_api GET "/emails/$1/attachments" | pretty
    ;;

  attachment)
    [[ $# -ge 2 ]] || err "usage: $0 attachment <email_id> <attachment_id>"
    resend_api GET "/emails/$1/attachments/$2" | pretty
    ;;

  *) err "unknown action: $action  (try: send|batch|ls|get|cancel|reschedule|attachments|attachment)" ;;
esac

