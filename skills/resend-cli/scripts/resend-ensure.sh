#!/usr/bin/env bash
# Read-only preflight for the Codex Resend CLI skill.
set -euo pipefail

ok() { printf 'OK: %s\n' "$1"; }
fail() { printf 'ERROR: %s\n' "$1" >&2; }
warn() { printf 'WARN: %s\n' "$1" >&2; }

errors=0

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"

if [[ -n "${BASH_VERSION:-}" ]]; then
  ok "bash ${BASH_VERSION%%(*}"
else
  fail "this skill requires Bash"
  errors=$((errors + 1))
fi

if command -v bun >/dev/null 2>&1; then
  ok "bun $(bun --version)"
  if version=$(bunx --bun resend-cli@latest --version 2>/dev/null | tail -n1); then
    ok "$version"
  else
    fail "bunx could not run resend-cli@latest"
    errors=$((errors + 1))
  fi
else
  fail "bun is required to run resend-cli without a global installation"
  errors=$((errors + 1))
fi

if command -v jq >/dev/null 2>&1; then
  ok "jq $(jq --version)"
else
  fail "jq is required by the bundled REST helpers"
  errors=$((errors + 1))
fi

if command -v curl >/dev/null 2>&1; then
  ok "curl $(curl --version | head -n1 | awk '{print $2}')"
else
  fail "curl is required by the bundled REST helpers"
  errors=$((errors + 1))
fi

key_available=false
key_source="${_RESEND_LOADED_FROM:-shell environment}"
if [[ -n "${RESEND_API_KEY:-}" ]]; then
  key_available=true
  ok "RESEND_API_KEY is set (${#RESEND_API_KEY} chars), source: $key_source"
  [[ "$RESEND_API_KEY" == re_* ]] || warn "RESEND_API_KEY does not start with 're_'; this may be an intentional test stub"
else
  fail "RESEND_API_KEY is not exported and no project .env.local or .env supplied it"
  errors=$((errors + 1))
fi

if [[ "$RESEND_HOST" == "https://api.resend.com" ]]; then
  ok "RESEND_HOST default ($RESEND_HOST)"
else
  warn "RESEND_HOST is overridden: $RESEND_HOST"
fi

if [[ -n "${RESEND_FROM:-}" ]]; then
  ok "RESEND_FROM is set"
else
  warn "RESEND_FROM is unset; send commands need an explicit --from"
fi

if [[ "$key_available" == true ]] && command -v curl >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
  if domains=$(resend_api GET "/domains" 2>/dev/null); then
    count=$(printf '%s' "$domains" | jq -r '.data | length // 0')
    ok "authenticated against $RESEND_HOST; team has $count domain(s)"
    if [[ "$count" -gt 0 ]]; then
      printf '%s' "$domains" | jq -r '.data[] | "  \(.name) [\(.status)] id=\(.id)"'
    fi
  else
    fail "authentication failed against $RESEND_HOST/domains"
    errors=$((errors + 1))
  fi

  if resend_api GET "/api-keys" >/dev/null 2>&1; then
    ok "key can access API-key management and is likely full_access"
  else
    warn "key cannot list API keys; it may be sending_access or otherwise restricted"
  fi
fi

printf '\n'
if [[ $errors -eq 0 ]]; then
  ok "all checks passed"
  exit 0
fi

fail "$errors check(s) failed"
exit 1
