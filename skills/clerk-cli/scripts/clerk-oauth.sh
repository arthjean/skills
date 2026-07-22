#!/usr/bin/env bash
# clerk-oauth.sh - OAuth Application CRUD + SAML connection CRUD + sign-in/actor token minting.
# These are advanced auth operations the agent-toolkit MCP does not expose.
#
# Usage:
#   ./clerk-oauth.sh apps-ls
#   ./clerk-oauth.sh apps-get        <id>
#   ./clerk-oauth.sh apps-create     <name> <redirect_uris_csv> [scopes_csv] [public]
#   ./clerk-oauth.sh apps-update     <id> <patch-json>
#   ./clerk-oauth.sh apps-rotate     <id>
#   ./clerk-oauth.sh apps-rm         <id>
#   ./clerk-oauth.sh saml-ls
#   ./clerk-oauth.sh saml-get        <id>
#   ./clerk-oauth.sh saml-create     <name> <domain> <idp_entity_id> <idp_sso_url>
#   ./clerk-oauth.sh saml-rm         <id>
#   ./clerk-oauth.sh signin-token    <user_id> [expires_in_seconds]
#   ./clerk-oauth.sh actor-token     <user_id> <actor_user_id> [expires_in_seconds]
#   ./clerk-oauth.sh testing-token

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 <action> [args...]"
action="$1"; shift

# Helper to convert CSV to JSON array
csv_to_json_array() {
  printf '%s' "$1" | tr ',' '\n' | jq -R . | jq -s 'map(select(length > 0))'
}

case "$action" in
  apps-ls)
    clerk_api GET "/oauth_applications" | jq .
    ;;

  apps-get)
    [[ $# -ge 1 ]] || err "usage: $0 apps-get <id>"
    clerk_api GET "/oauth_applications/$1" | jq .
    ;;

  apps-create)
    [[ $# -ge 2 ]] || err "usage: $0 apps-create <name> <redirect_uris_csv> [scopes_csv] [public]"
    name="$1"; redirs=$(csv_to_json_array "$2")
    scopes_arr="[]"
    [[ -n "${3:-}" ]] && scopes_arr=$(csv_to_json_array "$3")
    public="${4:-false}"
    body=$(jq -nc \
      --arg name "$name" \
      --argjson redirs "$redirs" \
      --argjson scopes "$scopes_arr" \
      --argjson public "$public" \
      '{name: $name, redirect_uris: $redirs, scopes: $scopes, public: $public}')
    clerk_api POST "/oauth_applications" "$body" | jq .
    ;;

  apps-update)
    [[ $# -ge 2 ]] || err "usage: $0 apps-update <id> <patch-json>"
    clerk_api PATCH "/oauth_applications/$1" "$2" | jq .
    ;;

  apps-rotate)
    [[ $# -ge 1 ]] || err "usage: $0 apps-rotate <id>"
    clerk_api POST "/oauth_applications/$1/rotate_secret" | jq .
    ;;

  apps-rm)
    [[ $# -ge 1 ]] || err "usage: $0 apps-rm <id>"
    clerk_api DELETE "/oauth_applications/$1" | jq .
    ;;

  saml-ls)
    clerk_api GET "/saml_connections" | jq .
    ;;

  saml-get)
    [[ $# -ge 1 ]] || err "usage: $0 saml-get <id>"
    clerk_api GET "/saml_connections/$1" | jq .
    ;;

  saml-create)
    [[ $# -ge 4 ]] || err "usage: $0 saml-create <name> <domain> <idp_entity_id> <idp_sso_url>"
    body=$(jq -nc --arg n "$1" --arg d "$2" --arg eid "$3" --arg sso "$4" \
      '{name: $n, domain: $d, idp_entity_id: $eid, idp_sso_url: $sso, provider: "saml_custom"}')
    clerk_api POST "/saml_connections" "$body" | jq .
    ;;

  saml-rm)
    [[ $# -ge 1 ]] || err "usage: $0 saml-rm <id>"
    clerk_api DELETE "/saml_connections/$1" | jq .
    ;;

  signin-token)
    [[ $# -ge 1 ]] || err "usage: $0 signin-token <user_id> [expires_in_seconds]"
    if [[ $# -ge 2 ]]; then
      body=$(jq -nc --arg uid "$1" --argjson e "$2" '{user_id: $uid, expires_in_seconds: $e}')
    else
      body=$(jq -nc --arg uid "$1" '{user_id: $uid}')
    fi
    clerk_api POST "/sign_in_tokens" "$body" | jq .
    ;;

  actor-token)
    [[ $# -ge 2 ]] || err "usage: $0 actor-token <user_id> <actor_user_id> [expires_in_seconds]"
    actor=$(jq -nc --arg sub "$2" '{sub: $sub}')
    if [[ $# -ge 3 ]]; then
      body=$(jq -nc --arg uid "$1" --argjson a "$actor" --argjson e "$3" \
        '{user_id: $uid, actor: $a, expires_in_seconds: $e}')
    else
      body=$(jq -nc --arg uid "$1" --argjson a "$actor" '{user_id: $uid, actor: $a}')
    fi
    clerk_api POST "/actor_tokens" "$body" | jq .
    ;;

  testing-token)
    clerk_api POST "/testing_tokens" '{}' | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
