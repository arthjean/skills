#!/usr/bin/env bash
# clerk-ensure.sh - preflight check for the clerk-cli skill.
# Verifies bun, clerk CLI reachability, jq, curl, CLERK_SECRET_KEY, API version, and live auth.
# Exits non-zero with a clear error per missing dependency.
set -euo pipefail

ok()   { printf '\033[32m✓\033[0m %s\n' "$1"; }
fail() { printf '\033[31m✗\033[0m %s\n' "$1" >&2; }
warn() { printf '\033[33m!\033[0m %s\n' "$1"; }

errors=0

# Source _lib.sh so the .env auto-loader runs before we check CLERK_SECRET_KEY.
# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"

# 1. bun (required for bunx)
if command -v bun >/dev/null 2>&1; then
  ok "bun ($(bun --version))"
else
  fail "bun not found. Install: curl -fsSL https://bun.sh/install | bash"
  errors=$((errors + 1))
fi

# 2. Clerk CLI reachable via bunx (the current package is `clerk`)
if command -v bun >/dev/null 2>&1; then
  if version=$(bunx clerk@latest --version 2>/dev/null | tail -n1); then
    ok "clerk ($version)"
  else
    warn "bunx clerk@latest --version failed. The CLI is optional; direct curl helpers still work."
  fi
fi

# 3. jq
if command -v jq >/dev/null 2>&1; then
  ok "jq ($(jq --version))"
else
  fail "jq not found. Install it with the host system package manager."
  errors=$((errors + 1))
fi

# 4. curl
if command -v curl >/dev/null 2>&1; then
  ok "curl ($(curl --version | head -n1 | awk '{print $2}'))"
else
  fail "curl not found. Install it with the host system package manager."
  errors=$((errors + 1))
fi

# 5. CLERK_SECRET_KEY (auto-loaded from .env(.local) by _lib.sh if not exported)
src="${_CLERK_LOADED_FROM:-shell environment}"
if [[ -n "${CLERK_SECRET_KEY:-}" ]]; then
  case "$CLERK_SECRET_KEY" in
    sk_test_*)
      ok "CLERK_SECRET_KEY is set (DEVELOPMENT instance, ${#CLERK_SECRET_KEY} chars) - source: $src"
      ;;
    sk_live_*)
      ok "CLERK_SECRET_KEY is set (PRODUCTION instance, ${#CLERK_SECRET_KEY} chars) - source: $src"
      ;;
    *)
      fail "CLERK_SECRET_KEY is set but format is wrong (expected 'sk_test_...' or 'sk_live_...')."
      errors=$((errors + 1))
      ;;
  esac
else
  fail "CLERK_SECRET_KEY not found. Three options (highest precedence first):"
  fail "  1. export CLERK_SECRET_KEY=your_clerk_secret_key in your shell"
  fail "  2. cd into a project with .env.local containing CLERK_SECRET_KEY=..."
  fail "  3. run 'bunx clerk@latest env pull' inside a Clerk-linked project"
  errors=$((errors + 1))
fi

# 6. API version
api_version="${CLERK_API_VERSION:-2026-05-12}"
if [[ -n "${CLERK_API_VERSION:-}" ]]; then
  ok "CLERK_API_VERSION explicitly pinned to $api_version"
else
  ok "CLERK_API_VERSION default ($api_version)"
fi

# 7. Live auth (1 cheap API call) - confirms the key works AND prints instance metadata
if [[ -n "${CLERK_SECRET_KEY:-}" ]] && command -v curl >/dev/null 2>&1; then
  if instance=$(curl -fsS \
      -H "Authorization: Bearer $CLERK_SECRET_KEY" \
      -H "Clerk-API-Version: $api_version" \
      https://api.clerk.com/v1/instance 2>/dev/null); then
    env_type=$(echo "$instance" | jq -r '.environment_type // "unknown"')
    home_origin=$(echo "$instance" | jq -r '.home_origin // "unset"')
    ok "authenticated - environment: $env_type, home_origin: $home_origin"
  else
    fail "CLERK_SECRET_KEY is set but auth failed. The key may be invalid, revoked, or for a different instance."
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
