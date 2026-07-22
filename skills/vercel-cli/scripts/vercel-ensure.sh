#!/usr/bin/env bash
# vercel-ensure.sh - preflight check for the vercel-cli skill.
# Verifies bun, vercel CLI reachability, jq, curl, VERCEL_TOKEN, and live auth.
# Exits non-zero with a clear error per missing dependency.
set -euo pipefail

ok()   { printf '\033[32m✓\033[0m %s\n' "$1"; }
fail() { printf '\033[31m✗\033[0m %s\n' "$1" >&2; }
warn() { printf '\033[33m!\033[0m %s\n' "$1"; }

errors=0

# 1. bun (required for bunx)
if command -v bun >/dev/null 2>&1; then
  ok "bun ($(bun --version))"
else
  fail "bun not found. Install Bun using the method appropriate for this host."
  errors=$((errors + 1))
fi

# 2. vercel reachable via bunx
if command -v bun >/dev/null 2>&1; then
  if version=$(bunx vercel@latest --version 2>/dev/null | tail -n1); then
    ok "vercel ($version)"
  else
    fail "bunx vercel@latest failed. Check network or run: bunx vercel@latest --help"
    errors=$((errors + 1))
  fi
fi

# 3. jq (required by helper scripts for JSON parsing)
if command -v jq >/dev/null 2>&1; then
  ok "jq ($(jq --version))"
  else
    fail "jq not found. Install jq with this host's package manager."
  errors=$((errors + 1))
fi

# 4. curl (required for REST API calls)
if command -v curl >/dev/null 2>&1; then
  ok "curl ($(curl --version | head -n1 | awk '{print $2}'))"
else
  fail "curl not found. Install curl with this host's package manager."
  errors=$((errors + 1))
fi

# 5. VERCEL_TOKEN
if [[ -n "${VERCEL_TOKEN:-}" ]]; then
  ok "VERCEL_TOKEN is set (${#VERCEL_TOKEN} chars)"
else
  fail "VERCEL_TOKEN is not set. Generate at https://vercel.com/account/tokens and:"
  fail "  export VERCEL_TOKEN=vcp_xxxxxxxxxxxxxxxxxxxxxxxx"
  errors=$((errors + 1))
fi

# 6. Optional: VERCEL_TEAM_ID
if [[ -n "${VERCEL_TEAM_ID:-}" ]]; then
  ok "VERCEL_TEAM_ID is set ($VERCEL_TEAM_ID)"
elif [[ -n "${VERCEL_ORG_ID:-}" ]]; then
  ok "VERCEL_ORG_ID is set ($VERCEL_ORG_ID)"
else
  warn "no VERCEL_TEAM_ID/VERCEL_ORG_ID set - personal scope assumed."
  warn "  For team-scoped operations: export VERCEL_TEAM_ID=team_xxx"
fi

# 7. Optional: VERCEL_PROJECT_ID or .vercel/project.json
if [[ -n "${VERCEL_PROJECT_ID:-}" ]]; then
  ok "VERCEL_PROJECT_ID is set ($VERCEL_PROJECT_ID)"
elif [[ -f .vercel/project.json ]]; then
  ctx_pid=$(jq -r '.projectId // empty' .vercel/project.json 2>/dev/null || true)
  if [[ -n "$ctx_pid" ]]; then
    ok "linked project: $ctx_pid (from .vercel/project.json)"
  fi
else
  warn "no project pinned. Pin one with:"
  warn "  bunx vercel@latest link --yes"
  warn "  (or pass --project on every command, or set VERCEL_PROJECT_ID env var)"
fi

# 8. Verify auth actually works (1 cheap API call)
if [[ -n "${VERCEL_TOKEN:-}" ]] && command -v curl >/dev/null 2>&1; then
  auth_config=$(printf 'header = "Authorization: Bearer %s"\n' "$VERCEL_TOKEN")
  if me=$(curl -fsS --config /dev/fd/3 https://api.vercel.com/v2/user 3<<<"$auth_config" 2>/dev/null); then
    username=$(echo "$me" | jq -r '.user.username // .user.email // "unknown"')
    ok "authenticated as: $username"
  else
    fail "VERCEL_TOKEN is set but auth failed. The token may be invalid, revoked, or scoped to a team that excludes the user endpoint."
    errors=$((errors + 1))
  fi
fi

echo
if [[ $errors -eq 0 ]]; then
  ok "all checks passed"
  exit 0
else
  fail "$errors check(s) failed"
  exit 1
fi
