#!/usr/bin/env bash
# posthog-api.sh - generic authenticated REST caller for the PostHog API.
# Auto-injects Authorization (Bearer phx_*) and resolves $POSTHOG_HOST. Retries on 429.
#
# Usage:
#   ./posthog-api.sh GET    /api/projects/
#   ./posthog-api.sh GET    "/api/projects/123/feature_flags/?limit=50&offset=0"
#   ./posthog-api.sh POST   /api/projects/123/feature_flags/ '{"key":"my-flag","name":"My flag","active":true}'
#   ./posthog-api.sh PATCH  /api/projects/123/feature_flags/456/ '{"active":false}'
#   ./posthog-api.sh DELETE /api/projects/123/cohorts/789/
#
# Output: pretty-printed JSON via jq.

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 2 ]] || err "usage: $0 <METHOD> <PATH> [json_body]"

method="$1"; path="$2"
shift 2

posthog_api "$method" "$path" "$@" | pretty
