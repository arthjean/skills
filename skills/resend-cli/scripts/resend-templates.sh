#!/usr/bin/env bash
# resend-templates.sh - reusable email templates (used by broadcasts and direct sends).
# MCP tools replaced: create_template, list_templates, get_template, update_template,
#                     publish_template, duplicate_template, remove_template
#
# Subcommands:
#   create     Create a template
#                Flags: --name <name> --subject <s> --html <str|@file> [--text <str|@file>]
#                       [--from <addr>] [--reply-to <addr>] [--description <text>]
#   ls         List templates
#   get        Get a single template       get <id>
#   update     Update a template           update <id> [flags...]
#   publish    Publish a draft template    publish <id>
#   duplicate  Duplicate a template        duplicate <id> [--name <new_name>]
#   rm         Delete a template           rm <id>

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|publish|duplicate|rm} [args...]"
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
    name=""; subject=""; html=""; text=""; from=""; reply_to=""; desc=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        name="$2"; shift 2 ;;
        --subject)     subject="$2"; shift 2 ;;
        --html)        html=$(_read_inline_or_file "$2"); shift 2 ;;
        --text)        text=$(_read_inline_or_file "$2"); shift 2 ;;
        --from)        from="$2"; shift 2 ;;
        --reply-to)    reply_to="$2"; shift 2 ;;
        --description) desc="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name"
    [[ -n "$subject" ]] || err "missing --subject"
    [[ -n "$html" || -n "$text" ]] || err "must supply --html or --text"
    body=$(jq -nc --arg n "$name" --arg s "$subject" '{name:$n, subject:$s}')
    [[ -n "$html" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$html"     '. + {html: $v}')
    [[ -n "$text" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$text"     '. + {text: $v}')
    [[ -n "$from" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$from"     '. + {from: $v}')
    [[ -n "$reply_to" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$reply_to" '. + {reply_to: $v}')
    [[ -n "$desc" ]]     && body=$(printf '%s' "$body" | jq -c --arg v "$desc"     '. + {description: $v}')
    resend_api POST "/templates" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/templates" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <template_id>"
    resend_api GET "/templates/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <template_id> [flags...]"
    tid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {name: $v}'); shift 2 ;;
        --subject)     body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {subject: $v}'); shift 2 ;;
        --html)        v=$(_read_inline_or_file "$2"); body=$(printf '%s' "$body" | jq -c --arg v "$v" '. + {html: $v}'); shift 2 ;;
        --text)        v=$(_read_inline_or_file "$2"); body=$(printf '%s' "$body" | jq -c --arg v "$v" '. + {text: $v}'); shift 2 ;;
        --from)        body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {from: $v}'); shift 2 ;;
        --reply-to)    body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {reply_to: $v}'); shift 2 ;;
        --description) body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {description: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/templates/$tid" "$body" | pretty
    ;;

  publish)
    [[ $# -ge 1 ]] || err "usage: $0 publish <template_id>"
    resend_api POST "/templates/$1/publish" | pretty
    ;;

  duplicate)
    [[ $# -ge 1 ]] || err "usage: $0 duplicate <template_id> [--name <new_name>]"
    tid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name) body=$(jq -nc --arg v "$2" '{name: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    if [[ "$body" == "{}" ]]; then
      resend_api POST "/templates/$tid/duplicate" | pretty
    else
      resend_api POST "/templates/$tid/duplicate" "$body" | pretty
    fi
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <template_id>"
    resend_api DELETE "/templates/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|publish|duplicate|rm)" ;;
esac

