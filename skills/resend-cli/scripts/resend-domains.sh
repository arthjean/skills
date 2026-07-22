#!/usr/bin/env bash
# resend-domains.sh - sending domains (DKIM, SPF, DMARC, verification).
# MCP tools replaced: create_domain, list_domains, get_domain, update_domain, remove_domain, verify_domain
#
# Subcommands:
#   create    Create a domain
#               Flags: --name <domain> [--region <us-east-1|eu-west-1|sa-east-1|ap-northeast-1>]
#                      [--click-tracking <on|off>] [--open-tracking <on|off>]
#   ls        List all domains
#   get       Get a single domain      get <id>
#   update    Update a domain          update <id> [--click-tracking on|off] [--open-tracking on|off]
#   verify    Trigger DNS verification verify <id>
#   rm        Delete a domain          rm <id>
#   dns       Print DNS records needed for a domain (extracted from get)

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 1 ]] || err "usage: $0 {create|ls|get|update|verify|rm|dns} [args...]"
action="$1"; shift

_bool() { case "${1:-}" in on|true|1|yes) echo true ;; off|false|0|no) echo false ;; *) err "expected on|off, got: $1" ;; esac; }

case "$action" in
  create)
    name=""; region=""; click=""; open=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --name)            name="$2"; shift 2 ;;
        --region)          region="$2"; shift 2 ;;
        --click-tracking)  click=$(_bool "$2"); shift 2 ;;
        --open-tracking)   open=$(_bool "$2"); shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    [[ -n "$name" ]] || err "missing --name <domain.com>"
    body=$(jq -nc --arg n "$name" '{name: $n}')
    [[ -n "$region" ]] && body=$(printf '%s' "$body" | jq -c --arg v "$region" '. + {region: $v}')
    [[ -n "$click" ]]  && body=$(printf '%s' "$body" | jq -c --argjson v "$click" '. + {click_tracking: $v}')
    [[ -n "$open" ]]   && body=$(printf '%s' "$body" | jq -c --argjson v "$open" '. + {open_tracking: $v}')
    resend_api POST "/domains" "$body" | pretty
    ;;

  ls|list)
    resend_api GET "/domains" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <domain_id>"
    resend_api GET "/domains/$1" | pretty
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <domain_id> [--click-tracking on|off] [--open-tracking on|off]"
    did="$1"; shift
    click=""; open=""; tls=""
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --click-tracking) click=$(_bool "$2"); shift 2 ;;
        --open-tracking)  open=$(_bool "$2"); shift 2 ;;
        --tls)            tls="$2"; shift 2 ;;
        *) err "unknown flag: $1" ;;
      esac
    done
    body='{}'
    [[ -n "$click" ]] && body=$(printf '%s' "$body" | jq -c --argjson v "$click" '. + {click_tracking: $v}')
    [[ -n "$open" ]]  && body=$(printf '%s' "$body" | jq -c --argjson v "$open"  '. + {open_tracking: $v}')
    [[ -n "$tls" ]]   && body=$(printf '%s' "$body" | jq -c --arg v "$tls" '. + {tls: $v}')
    [[ "$body" == "{}" ]] && err "nothing to update - pass at least one flag"
    resend_api PATCH "/domains/$did" "$body" | pretty
    ;;

  verify)
    [[ $# -ge 1 ]] || err "usage: $0 verify <domain_id>"
    resend_api POST "/domains/$1/verify" | pretty
    ;;

  rm|delete)
    [[ $# -ge 1 ]] || err "usage: $0 rm <domain_id>"
    resend_api DELETE "/domains/$1" | pretty
    ;;

  dns)
    [[ $# -ge 1 ]] || err "usage: $0 dns <domain_id>"
    resend_api GET "/domains/$1" | jq -r '
      "Domain: \(.name)  [\(.status)]",
      "",
      "DNS records to add at your registrar:",
      (.records[]? | "  \(.type)  \(.name)  →  \(.value)\(if .priority then "  (priority \(.priority))" else "" end)")
    '
    ;;

  *) err "unknown action: $action  (try: create|ls|get|update|verify|rm|dns)" ;;
esac

