#!/usr/bin/env bash

set -euo pipefail

ok() {
  printf '[ok] %s\n' "$1"
}

fail() {
  printf '[fail] %s\n' "$1" >&2
}

warn() {
  printf '[warn] %s\n' "$1"
}

find_neon_context() {
  local directory="$PWD"

  while true; do
    if [[ -f "$directory/.neon" ]]; then
      printf '%s\n' "$directory/.neon"
      return 0
    fi

    [[ "$directory" == "/" ]] && return 1
    directory="$(dirname "$directory")"
  done
}

errors=0

if command -v bun >/dev/null 2>&1; then
  ok "bun $(bun --version)"
else
  fail "bun is missing"
  errors=$((errors + 1))
fi

if command -v bun >/dev/null 2>&1; then
  if version="$(env -u NEON_API_KEY bunx neonctl@latest --version 2>/dev/null | tail -n 1)"; then
    ok "neonctl $version"
  else
    fail "bunx neonctl@latest failed"
    errors=$((errors + 1))
  fi
fi

if command -v psql >/dev/null 2>&1; then
  ok "psql $(psql --version | awk '{print $3}')"
else
  fail "psql is missing. Install the PostgreSQL client for this operating system."
  errors=$((errors + 1))
fi

if command -v jq >/dev/null 2>&1; then
  ok "jq $(jq --version)"
else
  fail "jq is missing. Install it with this operating system's package manager."
  errors=$((errors + 1))
fi

if [[ -n "${NEON_API_KEY:-}" ]]; then
  ok "NEON_API_KEY is set"
else
  fail "NEON_API_KEY is not set"
  errors=$((errors + 1))
fi

if [[ -n "${NEON_PROJECT_ID:-}" ]]; then
  ok "project target supplied through NEON_PROJECT_ID"
elif context_file="$(find_neon_context)"; then
  ok "local Neon context found at $context_file"
else
  warn "no project target found. Pass a project ID, set NEON_PROJECT_ID, or link this repository with neonctl link."
fi

# Keep identity and account fields out of logs while still verifying the key.
if [[ -n "${NEON_API_KEY:-}" ]] && command -v bun >/dev/null 2>&1; then
  if bunx neonctl@latest me --output json >/dev/null 2>&1; then
    ok "Neon authentication succeeded"
  else
    fail "Neon authentication failed. The API key may be invalid, revoked, or out of scope."
    errors=$((errors + 1))
  fi
fi

printf '\n'

if [[ "$errors" -eq 0 ]]; then
  ok "all checks passed"
  exit 0
fi

fail "$errors check(s) failed"
exit 1
