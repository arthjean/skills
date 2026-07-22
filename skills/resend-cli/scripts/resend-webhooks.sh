#!/usr/bin/env bash
# resend-webhooks.sh - webhook endpoints for delivery events (delivered, bounced, opened, clicked, ...).
# MCP tools replaced: create_webhook, list_webhooks, get_webhook, update_webhook, remove_webhook
#
# Subcommands:
#   create   Create a webhook
#              Flags: --url <https://...> --events <evt,evt,...>   (comma-separated event types)
#                     [--name <internal>] [--enabled true|false] --out <secure-json-path>
#   ls       List webhooks
#   get      Get a single webhook                  get <id>
#   update   Update a webhook                      update <id> [--url ...] [--events ...] [--enabled true|false]
#   rm       Delete a webhook                      rm <id>
#
#   listen   Run the official CLI tunnel locally (delegates to `bunx --bun resend-cli@latest`).
#            Use when you want to test webhook delivery to localhost without ngrok.
#            Usage: resend-webhooks.sh listen [--port 3000]
#
# Common event types: email.sent, email.delivered, email.delivery_delayed, email.complained,
#                     email.bounced, email.opened, email.clicked, email.failed,
#                     contact.created, contact.updated, contact.deleted,
#                     domain.created, domain.updated, domain.deleted

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|rm|listen} [args...]"
action="$1"; shift

_csv_to_json_array() {
  local csv="${1:-}"
  [[ -z "$csv" ]] && printf '[]' && return 0
  local IFS=','; read -ra arr <<< "$csv"
  printf '%s' "$(printf '%s\n' "${arr[@]}" | jq -R . | jq -s .)"
}

_bool() { case "${1:-}" in true|1|yes|on) echo true ;; false|0|no|off) echo false ;; *) err "expected true|false, got: $1" ;; esac; }

case "$action" in
  create)
    require_resend_key
    url=""; events=""; name=""; enabled=""; out=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --url)     url="$2"; shift 2 ;;
        --events)  events="$2"; shift 2 ;;
        --name)    name="$2"; shift 2 ;;
        --enabled) enabled=$(_bool "$2"); shift 2 ;;
        --out)     out="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$url" ]] || err "missing --url"
    [[ -n "$events" ]] || err "missing --events <evt,evt,...>"
    [[ -n "$out" ]] || err "missing --out <secure-json-path>; webhook creation can return a one-time signing secret"
    [[ ! -e "$out" ]] || err "refusing to overwrite existing path: $out"
    evts_arr=$(_csv_to_json_array "$events")
    body=$(jq -nc --arg u "$url" --argjson e "$evts_arr" '{endpoint:$u, events:$e}')
    [[ -n "$name" ]]    && body=$(printf '%s' "$body" | jq -c --arg v "$name" '. + {name: $v}')
    [[ -n "$enabled" ]] && body=$(printf '%s' "$body" | jq -c --argjson v "$enabled" '. + {enabled: $v}')
    response=$(resend_api POST "/webhooks" "$body")
    old_umask=$(umask)
    umask 077
    printf '%s\n' "$response" > "$out"
    umask "$old_umask"
    printf '%s' "$response" | jq --arg saved_to "$out" 'del(.signing_secret, .secret) + {secret_response_saved_to: $saved_to}'
    ;;

  ls|list)
    require_resend_key
    resend_api GET "/webhooks" | pretty
    ;;

  get)
    require_resend_key
    [[ $# -ge 1 ]] || err "usage: $0 get <webhook_id>"
    resend_api GET "/webhooks/$1" | pretty
    ;;

  update)
    require_resend_key
    [[ $# -ge 1 ]] || err "usage: $0 update <webhook_id> [flags...]"
    wid="$1"; shift
    body='{}'
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --url)     body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {endpoint: $v}'); shift 2 ;;
        --events)  evts_arr=$(_csv_to_json_array "$2"); body=$(printf '%s' "$body" | jq -c --argjson v "$evts_arr" '. + {events: $v}'); shift 2 ;;
        --name)    body=$(printf '%s' "$body" | jq -c --arg v "$2" '. + {name: $v}'); shift 2 ;;
        --enabled) v=$(_bool "$2"); body=$(printf '%s' "$body" | jq -c --argjson v "$v" '. + {enabled: $v}'); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/webhooks/$wid" "$body" | pretty
    ;;

  rm|delete)
    require_resend_key
    [[ $# -ge 1 ]] || err "usage: $0 rm <webhook_id>"
    resend_api DELETE "/webhooks/$1" | pretty
    ;;

  listen)
    # Delegate to the official CLI because the REST API cannot expose a local tunnel.
    command -v bun >/dev/null 2>&1 || err "listen requires bun + bunx resend-cli"
    require_resend_key
    exec bunx --bun resend-cli@latest webhooks listen "$@"
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|rm|listen)" ;;
esac
