#!/usr/bin/env bash
# clerk-api.sh - generic authenticated REST caller for api.clerk.com/v1.
# Uses the same Backend API + secret key as @clerk/agent-toolkit and the official `clerk` CLI.
# Auto-injects Authorization, Clerk-API-Version, and Content-Type. Retries once on 429.
#
# Usage:
#   ./clerk-api.sh GET    /users
#   ./clerk-api.sh GET    "/users?limit=50&offset=0&order_by=-created_at"
#   ./clerk-api.sh POST   /organizations '{"name":"Acme","created_by":"user_xxx"}'
#   ./clerk-api.sh PATCH  /users/user_xxx '{"public_metadata":{"plan":"pro"}}'
#   ./clerk-api.sh POST   /sessions/sess_xxx/revoke
#
# Output: pretty-printed JSON via jq.

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 2 ]] || err "usage: $0 <METHOD> <PATH> [json_body]"

method="$1"; path="$2"
shift 2

response=$(clerk_api "$method" "$path" "$@")
if printf '%s' "$response" | jq empty >/dev/null 2>&1; then
  printf '%s' "$response" | jq .
else
  printf '%s' "$response"
fi
