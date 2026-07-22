#!/usr/bin/env bash
# resend-topics.sh - subscription topics (e.g., "marketing", "product updates").
# MCP tools replaced: create_topic, list_topics, get_topic, update_topic, remove_topic
#
# Subcommands:
#   create   Create a topic
#              Flags: --name <name> [--description <text>] [--default-subscribed]
#   ls       List topics
#   get      Get a single topic        get <id>
#   update   Update a topic            update <id> [--name <name>] [--description <text>]
#   rm       Delete a topic            rm <id>

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|rm} [args...]"
action="$1"; shift

case "$action" in
  create)
    name=""; desc=""; default_sub=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)               name="$2"; shift 2 ;;
        --description)        desc="$2"; shift 2 ;;
        --default-subscribed) default_sub="true"; shift ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name"
    body=$(jq -nc --arg n "$name" '{name:$n}')
    [[ -n "$desc" ]]        && body=$(printf '%s' "$body" | jq -c --arg v "$desc" '. + {description: $v}')
    [[ -n "$default_sub" ]] && body=$(printf '%s' "$body" | jq -c '. + {default_subscribed: true}')
    resend_api POST "/topics" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/topics" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <topic_id>"
    resend_api GET "/topics/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <topic_id> [--name ...] [--description ...]"
    tid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {name: $v}'); shift 2 ;;
        --description) body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {description: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/topics/$tid" "$body" | pretty
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <topic_id>"
    resend_api DELETE "/topics/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|rm)" ;;
esac

