#!/usr/bin/env bash
# resend-api.sh - raw REST wrapper. Use when no resource script covers what you need.
#
# Usage: resend-api.sh METHOD PATH [json_body]
# Examples:
#   resend-api.sh GET    /domains
#   resend-api.sh GET    "/emails?limit=20"
#   resend-api.sh POST   /emails '{"from":"a@b.com","to":["c@d.com"],"subject":"hi","html":"<p>hi</p>"}'
#   resend-api.sh PATCH  /domains/abc-123 '{"name":"new-name"}'
#   resend-api.sh DELETE /api-keys/xyz
#
# For idempotent POSTs to /emails or /emails/batch, set RESEND_IDEMPOTENCY_KEY first:
#   RESEND_IDEMPOTENCY_KEY=signup-1234 resend-api.sh POST /emails "$body"

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
require_resend_key

[[ $# -ge 2 ]] || err "usage: $0 <METHOD> <PATH> [json_body]"

method="$1"; path="$2"; shift 2
resend_api "$method" "$path" "$@" | pretty

