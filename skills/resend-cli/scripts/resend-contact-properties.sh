#!/usr/bin/env bash
# resend-contact-properties.sh - custom contact properties schema CRUD.
# MCP tools replaced: create_contact_property, list_contact_properties, get_contact_property,
#                     update_contact_property, remove_contact_property
#
# Subcommands:
#   create   Create a property schema
#              Flags: --name <key> --type <string|number|boolean|date> [--description <text>]
#   ls       List properties
#   get      Get a single property         get <id>
#   update   Update a property             update <id> [--description <text>] [--name <key>]
#   rm       Delete a property             rm <id>

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|rm} [args...]"
action="$1"; shift

case "$action" in
  create)
    name=""; type=""; desc=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        name="$2"; shift 2 ;;
        --type)        type="$2"; shift 2 ;;
        --description) desc="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name"
    [[ -n "$type" ]] || err "missing --type <string|number|boolean|date>"
    case "$type" in string|number|boolean|date) ;; *) err "type must be string|number|boolean|date" ;; esac
    body=$(jq -nc --arg n "$name" --arg t "$type" '{name:$n, type:$t}')
    [[ -n "$desc" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$desc" '. + {description: $v}')
    resend_api POST "/contact-properties" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/contact-properties" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <property_id>"
    resend_api GET "/contact-properties/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <property_id> [--name ...] [--description ...]"
    pid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)        body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {name: $v}'); shift 2 ;;
        --description) body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {description: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/contact-properties/$pid" "$body" | pretty
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <property_id>"
    resend_api DELETE "/contact-properties/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|rm)" ;;
esac

