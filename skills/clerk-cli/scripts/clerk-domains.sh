#!/usr/bin/env bash
# clerk-domains.sh - instance-level domain CRUD + redirect URL CRUD.
# Backend API resources the agent-toolkit MCP does not expose.
#
# Usage:
#   ./clerk-domains.sh ls
#   ./clerk-domains.sh add        <name> [is_satellite]
#   ./clerk-domains.sh rm         <domain_id>
#   ./clerk-domains.sh redirects-ls
#   ./clerk-domains.sh redirect-add  <url>
#   ./clerk-domains.sh redirect-rm   <id>

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 <action> [args...]"
action="$1"; shift

case "$action" in
  ls)
    clerk_api GET "/domains" | jq .
    ;;

  add)
    [[ $# -ge 1 ]] || err "usage: $0 add <name> [is_satellite]"
    sat="${2:-false}"
    body=$(jq -nc --arg n "$1" --argjson s "$sat" '{name: $n, is_satellite: $s}')
    clerk_api POST "/domains" "$body" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <domain_id>"
    clerk_api DELETE "/domains/$1" | jq .
    ;;

  redirects-ls)
    clerk_api GET "/redirect_urls" | jq .
    ;;

  redirect-add)
    [[ $# -ge 1 ]] || err "usage: $0 redirect-add <url>"
    body=$(jq -nc --arg u "$1" '{url: $u}')
    clerk_api POST "/redirect_urls" "$body" | jq .
    ;;

  redirect-rm)
    [[ $# -ge 1 ]] || err "usage: $0 redirect-rm <id>"
    clerk_api DELETE "/redirect_urls/$1" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
