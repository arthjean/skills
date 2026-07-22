#!/usr/bin/env bash
# resend-contacts.sh - contacts CRUD + segment/topic memberships.
# MCP tools replaced: create_contact, list_contacts, get_contact, update_contact, remove_contact,
#                     add_contact_to_segment, remove_contact_from_segment, list_contact_segments,
#                     list_contact_topics, update_contact_topics
#
# Subcommands:
#   create     Create a contact
#                Flags: --email <a@b.com> [--first <name>] [--last <name>] [--unsubscribed]
#                       [--prop k=v ...]  (custom contact properties)
#   ls         List contacts (paginated)
#   get        Get a single contact          get <id>
#   update     Update a contact              update <id> [--first ...] [--last ...] [--unsubscribed true|false] [--prop k=v ...]
#   rm         Delete a contact              rm <id>
#
#   # Segment membership
#   segments              List segments a contact belongs to       segments <contact_id>
#   add-segment           Add contact to a segment                  add-segment <contact_id> <segment_id>
#   rm-segment            Remove contact from a segment             rm-segment <contact_id> <segment_id>
#
#   # Topic (subscription) membership
#   topics                List topic subscriptions                  topics <contact_id>
#   set-topics            Update topic subscriptions (JSON body)    set-topics <contact_id> <@file.json|json>
#
# Examples:
#   resend-contacts.sh create --email a@b.com --first Ada --last Lovelace --prop plan=pro
#   resend-contacts.sh ls | jq -s 'length'
#   resend-contacts.sh add-segment cnt_123 seg_456

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|rm|segments|add-segment|rm-segment|topics|set-topics} [args...]"
action="$1"; shift

# Build a {properties: {...}} object from --prop k=v repeatables; returns the inner object as compact JSON or empty.
_build_properties() {
  local -n _args=$1
  local props='{}'
  for kv in "${_args[@]}"; do
    name="${kv%%=*}"; value="${kv#*=}"
    [[ "$name" == "$value" || -z "$name" ]] && err "--prop must be name=value (got: $kv)"
    props=$(printf '%s' "$props" | jq -c --arg n "$name" --arg v "$value" '. + {($n): $v}')
  done
  printf '%s' "$props"
}

case "$action" in
  create)
    email=""; first=""; last=""; unsub=""
    declare -a prop_args=()
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --email)        email="$2"; shift 2 ;;
        --first)        first="$2"; shift 2 ;;
        --last)         last="$2"; shift 2 ;;
        --unsubscribed) unsub="true"; shift ;;
        --prop)         prop_args+=("$2"); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$email" ]] || err "missing --email"
    body=$(jq -nc --arg e "$email" '{email:$e}')
    [[ -n "$first" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$first" '. + {first_name: $v}')
    [[ -n "$last" ]]  && body=$(printf '%s' "$body" | jq -c --arg v "$last"  '. + {last_name: $v}')
    [[ -n "$unsub" ]] && body=$(printf '%s' "$body" | jq -c '. + {unsubscribed: true}')
    if [[ ${#prop_args[@]} -gt 0 ]]; then
      props=$(_build_properties prop_args)
      body=$(printf '%s' "$body" | jq -c --argjson v "$props" '. + {properties: $v}')
    fi
    resend_api POST "/contacts" "$body" | pretty
    ;;

  ls|list)
    limit="${RESEND_PAGE_LIMIT:-100}"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --limit) limit="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    resend_paginate "/contacts?limit=$limit"
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <contact_id>"
    resend_api GET "/contacts/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <contact_id> [flags...]"
    cid="$1"; shift
    first=""; last=""; unsub=""; email=""
    declare -a prop_args=()
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --email)        email="$2"; shift 2 ;;
        --first)        first="$2"; shift 2 ;;
        --last)         last="$2"; shift 2 ;;
        --unsubscribed) unsub="$2"; shift 2 ;;
        --prop)         prop_args+=("$2"); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    body='{}'
    [[ -n "$email" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$email" '. + {email: $v}')
    [[ -n "$first" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$first" '. + {first_name: $v}')
    [[ -n "$last" ]]  && body=$(printf '%s' "$body" | jq -c --arg v "$last"  '. + {last_name: $v}')
    if [[ -n "$unsub" ]]; then
      case "$unsub" in true|1|yes) body=$(printf '%s' "$body" | jq -c '. + {unsubscribed: true}') ;;
                       false|0|no) body=$(printf '%s' "$body" | jq -c '. + {unsubscribed: false}') ;;
                       *) err "--unsubscribed must be true|false" ;; esac
    fi
    if [[ ${#prop_args[@]} -gt 0 ]]; then
      props=$(_build_properties prop_args)
      body=$(printf '%s' "$body" | jq -c --argjson v "$props" '. + {properties: $v}')
    fi
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/contacts/$cid" "$body" | pretty
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <contact_id>"
    resend_api DELETE "/contacts/$1" | pretty
    ;;

  segments)
    [[ $# -ge 1 ]] || err "usage: $0 segments <contact_id>"
    resend_api GET "/contacts/$1/segments" | pretty
    ;;

  add-segment)
    [[ $# -ge 2 ]] || err "usage: $0 add-segment <contact_id> <segment_id>"
    body=$(jq -nc --arg s "$2" '{segment_id: $s}')
    resend_api POST "/contacts/$1/segments" "$body" | pretty
    ;;

  rm-segment)
    [[ $# -ge 2 ]] || err "usage: $0 rm-segment <contact_id> <segment_id>"
    resend_api DELETE "/contacts/$1/segments/$2" | pretty
    ;;

  topics)
    [[ $# -ge 1 ]] || err "usage: $0 topics <contact_id>"
    resend_api GET "/contacts/$1/topics" | pretty
    ;;

  set-topics)
    [[ $# -ge 2 ]] || err "usage: $0 set-topics <contact_id> <@file.json|json-body>"
    cid="$1"; arg="$2"
    if [[ "$arg" == @* ]]; then
      f="${arg:1}"; [[ -f "$f" ]] || err "file not found: $f"
      body=$(cat "$f")
    else
      body="$arg"
    fi
    resend_api PATCH "/contacts/$cid/topics" "$body" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|rm|segments|add-segment|rm-segment|topics|set-topics)" ;;
esac

