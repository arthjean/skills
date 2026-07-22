#!/usr/bin/env bash
# Pass event operations to the official CLI. The prior REST helper relied on inferred schema paths.
set -euo pipefail

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"

command -v bun >/dev/null 2>&1 || {
  printf 'ERROR: bun is required to run resend-cli@latest\n' >&2
  exit 1
}

[[ -n "${RESEND_API_KEY:-}" || -n "${RESEND_PROFILE:-}" ]] || {
  printf 'ERROR: export RESEND_API_KEY or select RESEND_PROFILE before running event commands\n' >&2
  exit 1
}

exec bunx --bun resend-cli@latest events "$@"
