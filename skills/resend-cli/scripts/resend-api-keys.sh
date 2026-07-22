#!/usr/bin/env bash
# resend-api-keys.sh - manage Resend API keys (requires a full_access key).
# MCP tools replaced: create_api_key, list_api_keys, remove_api_key
#
# Subcommands:
#   create   Create a new API key
#              Flags: --name <label> --permission <full_access|sending_access> [--domain <domain_id>] --out <path>
#              Notes: domain scoping only works with sending_access; the raw token is saved once to a mode-600 file.
#   ls       List all API keys (no token values shown - those are write-once)
#   rm       Delete an API key                rm <key_id>
#
# Examples:
#   resend-api-keys.sh create --name "CI/CD" --permission sending_access --domain dom-123 --out ./resend-key.json
#   resend-api-keys.sh ls | jq '.data[] | {id, name, permission, created_at}'
#   resend-api-keys.sh rm key_xyz

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|rm} [args...]"
action="$1"; shift

case "$action" in
  create)
    name=""; permission=""; domain=""; out=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)       name="$2"; shift 2 ;;
        --permission) permission="$2"; shift 2 ;;
        --domain)     domain="$2"; shift 2 ;;
        --out)        out="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name <label>"
    [[ -n "$permission" ]] || err "missing --permission <full_access|sending_access>"
    case "$permission" in full_access|sending_access) ;; *) err "permission must be full_access or sending_access (got: $permission)" ;; esac
    [[ -n "$domain" && "$permission" != "sending_access" ]] && err "--domain is only valid with --permission sending_access"
    [[ -n "$out" ]] || err "missing --out <secure-json-path>; the raw token is returned once and must not be printed"
    [[ ! -e "$out" ]] || err "refusing to overwrite existing path: $out"
    body=$(jq -nc --arg n "$name" --arg p "$permission" '{name:$n, permission:$p}')
    [[ -n "$domain" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$domain" '. + {domain_id: $v}')
    response=$(resend_api POST "/api-keys" "$body")
    old_umask=$(umask)
    umask 077
    printf '%s\n' "$response" > "$out"
    umask "$old_umask"
    printf '%s' "$response" | jq --arg saved_to "$out" 'del(.token) + {token_saved_to: $saved_to}'
    ;;

  ls|list)
    resend_api GET "/api-keys" | pretty
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <api_key_id>"
    resend_api DELETE "/api-keys/$1" | pretty
    ;;

  *) err "unknown action: $action  (try: create|ls|rm)" ;;
esac
