#!/usr/bin/env bash
# vercel-api.sh - generic authenticated REST caller for api.vercel.com.
# Replaces direct curl invocations and any MCP tool that maps to a single REST call.
#
# Usage:
#   ./vercel-api.sh <METHOD> <PATH> [json_body | @json_file | -]
#   ./vercel-api.sh GET  "/v2/user"
#   ./vercel-api.sh GET  "/v9/projects?limit=20"   # query strings are passed through
#   ./vercel-api.sh POST "/v1/webhooks" @webhook.json
#   printf '%s' "$JSON_BODY" | ./vercel-api.sh POST "/v1/webhooks" -
#   ./vercel-api.sh DELETE "/v1/webhooks/wh_xxx"
#
# Auto-injects: Authorization: Bearer $VERCEL_TOKEN, Accept: application/json,
# and Content-Type: application/json when a body is passed.
# Output is the raw JSON response (pretty-printed via jq if it parses as JSON).

source "$(dirname "$0")/_lib.sh"
require_vercel_token

[[ $# -ge 2 ]] || err "usage: $0 <METHOD> <PATH> [json_body]"

method="$1"
path="$2"
body_arg="${3:-}"
case "$body_arg" in
  @*)
    body_file="${body_arg#@}"
    [[ -f "$body_file" ]] || err "JSON body file not found: $body_file"
    body=$(<"$body_file")
    ;;
  -)
    body=$(</dev/stdin)
    ;;
  *)
    body="$body_arg"
    ;;
esac

# Auto-add ?teamId= unless the path already has one and a team is resolvable.
if [[ "$path" != *"teamId="* && "$path" != "/v2/user"* && "$path" != "/v2/teams"* ]]; then
  path=$(with_team_query "$path")
fi

if [[ -n "$body" ]]; then
  out=$(vercel_api "$method" "$path" "$body")
else
  out=$(vercel_api "$method" "$path")
fi

# Pretty-print if jq parses it; otherwise echo raw.
if echo "$out" | jq . >/dev/null 2>&1; then
  echo "$out" | jq .
else
  echo "$out"
fi
