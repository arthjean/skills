#!/usr/bin/env bash
# clerk-instance.sh - instance settings inspection and patch.
# Backend API resources the agent-toolkit MCP does not expose.
#
# Usage:
#   ./clerk-instance.sh get
#   ./clerk-instance.sh update         <patch-json>
#   ./clerk-instance.sh restrictions   <patch-json>
#   ./clerk-instance.sh org-settings   <patch-json>
#   ./clerk-instance.sh providers
#   ./clerk-instance.sh jwks
#
# Patch examples:
#   ./clerk-instance.sh restrictions '{"allowlist":true,"blocklist":false}'
#   ./clerk-instance.sh org-settings '{"max_allowed_memberships":50,"creator_role":"org:admin"}'
#   ./clerk-instance.sh update '{"home_origin":"https://app.example.com"}'

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 {get|update|restrictions|org-settings|providers|jwks} [patch-json]"
action="$1"; shift

case "$action" in
  get)
    clerk_api GET "/instance" | jq .
    ;;

  update)
    [[ $# -ge 1 ]] || err "usage: $0 update <patch-json>"
    clerk_api PATCH "/instance" "$1" | jq .
    ;;

  restrictions)
    [[ $# -ge 1 ]] || err "usage: $0 restrictions <patch-json>"
    clerk_api PATCH "/instance/restrictions" "$1" | jq .
    ;;

  org-settings)
    [[ $# -ge 1 ]] || err "usage: $0 org-settings <patch-json>"
    clerk_api PATCH "/instance/organization_settings" "$1" | jq .
    ;;

  providers)
    # Read through BAPI. Use the native config CLI for supported provider changes.
    clerk_api GET "/instance" | jq '.auth_config // {} | {social: .social_settings, oauth: .oauth_applications_enabled}'
    ;;

  jwks)
    # No rate limit on this endpoint.
    clerk_api GET "/jwks" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
