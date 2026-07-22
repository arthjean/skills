#!/usr/bin/env bash
# vercel-deploy.sh - deploy current directory and return structured JSON.
# Replaces MCP `deploy_to_vercel`. Wraps `vercel deploy` and parses
# the resulting URL into JSON {url, state, id, target} for downstream piping.
#
# Usage:
#   ./vercel-deploy.sh                   # preview deploy
#   ./vercel-deploy.sh --prod            # production deploy
#   ./vercel-deploy.sh --target=staging  # custom environment
#   ./vercel-deploy.sh --prod --project my-app
#
# Examples:
#   DEP=$(./vercel-deploy.sh --prod | jq -r '.url')
#   echo "https://$DEP"

source "$(dirname "$0")/_lib.sh"
require_vercel_token

# Resolve scope flags from env if present (CLI honors VERCEL_ORG_ID/VERCEL_PROJECT_ID natively
# but we also pass --scope explicitly for clarity if VERCEL_TEAM_ID is set).
scope_args=()
if [[ -n "${VERCEL_TEAM_ID:-}" ]]; then
  scope_args+=(--scope "$VERCEL_TEAM_ID")
fi

# Run the deploy. --yes auto-confirms project linking. stdout = URL, stderr = log.
# Capture both to surface useful errors.
deploy_log=$(mktemp)
trap 'rm -f "$deploy_log"' EXIT

if ! url=$(bunx vercel@latest deploy \
  --yes \
  "${scope_args[@]}" \
  "$@" 2>"$deploy_log"); then
  echo "deploy failed:" >&2
  cat "$deploy_log" >&2
  exit 1
fi

# Strip protocol if present and trailing whitespace.
url=$(echo "$url" | tail -n1 | sed 's|^https\?://||' | tr -d '[:space:]')

if [[ -z "$url" ]]; then
  echo "deploy succeeded but no URL was returned:" >&2
  cat "$deploy_log" >&2
  exit 1
fi

# Determine target from args.
target="preview"
for a in "$@"; do
  case "$a" in
    --prod) target="production" ;;
    --target=*) target="${a#--target=}" ;;
  esac
done

# Look up deployment ID + state via REST (the CLI doesn't return JSON for `deploy`).
# We resolve the deployment by URL.
detail_path=$(with_team_query "/v13/deployments/$url")
if detail=$(vercel_api GET "$detail_path" 2>/dev/null); then
  id=$(echo "$detail" | jq -r '.id // .uid // empty')
  state=$(echo "$detail" | jq -r '.readyState // .state // empty')
else
  id=""
  state="UNKNOWN"
fi

jq -n \
  --arg url "$url" \
  --arg id "$id" \
  --arg state "$state" \
  --arg target "$target" \
  '{url: $url, id: $id, state: $state, target: $target}'
