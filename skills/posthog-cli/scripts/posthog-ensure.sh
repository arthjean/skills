#!/usr/bin/env bash
# posthog-ensure.sh - preflight check for the posthog-cli skill.
# Verifies bun, @posthog/cli reachability, jq, curl, POSTHOG_PERSONAL_API_KEY, host, project ID,
# and a live auth call to GET /api/users/@me/.
# Exits non-zero with a clear error per missing dependency.
set -euo pipefail

ok()   { printf '\033[32m✓\033[0m %s\n' "$1"; }
fail() { printf '\033[31m✗\033[0m %s\n' "$1" >&2; }
warn() { printf '\033[33m!\033[0m %s\n' "$1"; }

errors=0

# shellcheck disable=SC1091
source "$(dirname "$0")/_lib.sh"
SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"

# 1. bun (required for bunx - global rule)
if command -v bun >/dev/null 2>&1; then
  ok "bun ($(bun --version))"
else
  fail "bun not found. Install: curl -fsSL https://bun.sh/install | bash"
  errors=$((errors + 1))
fi

# 2. @posthog/cli reachable via bunx (primary agent API and artifact CLI)
if command -v bun >/dev/null 2>&1; then
  if command -v timeout >/dev/null 2>&1; then
    version=$(timeout 30 bunx --bun @posthog/cli@latest --version 2>/dev/null | tail -n1) || true
  else
    version=$(bunx --bun @posthog/cli@latest --version 2>/dev/null | tail -n1) || true
  fi
  if [[ -n "$version" ]]; then
    ok "@posthog/cli ($version) - agent API and artifact operations"
  else
    fail "bunx --bun @posthog/cli@latest --version did not return."
    errors=$((errors + 1))
  fi
fi

# 3. jq
if command -v jq >/dev/null 2>&1; then
  ok "jq ($(jq --version))"
else
  fail "jq not found. Install jq with the host operating system's package manager."
  errors=$((errors + 1))
fi

# 4. curl
if command -v curl >/dev/null 2>&1; then
  ok "curl ($(curl --version | head -n1 | awk '{print $2}'))"
else
  fail "curl not found. Install curl with the host operating system's package manager."
  errors=$((errors + 1))
fi

# 5. POSTHOG_PERSONAL_API_KEY (auto-loaded from .env(.local) by _lib.sh if not exported)
src="${_POSTHOG_LOADED_FROM:-shell environment}"
if [[ -n "${POSTHOG_PERSONAL_API_KEY:-}" ]]; then
  case "$POSTHOG_PERSONAL_API_KEY" in
    phx_*)
      ok "POSTHOG_PERSONAL_API_KEY is set (personal key, ${#POSTHOG_PERSONAL_API_KEY} chars) - source: $src"
      ;;
    phc_*)
      fail "POSTHOG_PERSONAL_API_KEY looks like a project API key (phc_*), not a personal key (phx_*)."
      fail "  Project keys are public ingestion-only; they cannot read/write the management API."
      errors=$((errors + 1))
      ;;
    *)
      warn "POSTHOG_PERSONAL_API_KEY is set but does not start with 'phx_' (got '${POSTHOG_PERSONAL_API_KEY:0:8}...'). Older keys may use a different prefix - continuing."
      ;;
  esac
else
  fail "POSTHOG_PERSONAL_API_KEY not found. Three ways to provide it:"
  fail "  1. export POSTHOG_PERSONAL_API_KEY=phx_xxx in your shell"
  fail "  2. cd into a project with .env.local containing POSTHOG_PERSONAL_API_KEY=..."
  fail "  3. Create one at PostHog UI → Settings → Personal API Keys → '+ Create personal API key'"
  errors=$((errors + 1))
fi

# 6. POSTHOG_HOST
if [[ "$POSTHOG_HOST" == "https://us.posthog.com" ]]; then
  ok "POSTHOG_HOST default ($POSTHOG_HOST)"
else
  ok "POSTHOG_HOST overridden: $POSTHOG_HOST"
fi

# 7. POSTHOG_PROJECT_ID (optional - many scripts accept an explicit arg)
if [[ -n "${POSTHOG_PROJECT_ID:-}" ]]; then
  ok "POSTHOG_PROJECT_ID is set ($POSTHOG_PROJECT_ID)"
else
  warn "POSTHOG_PROJECT_ID not set. Project-scoped scripts will need an explicit project_id arg."
  warn "  Find IDs after auth: bash \"$SCRIPT_DIR/posthog-projects.sh\" ls"
fi

# 8. Live auth call - confirms the key works AND prints user/org/project context
if [[ -n "${POSTHOG_PERSONAL_API_KEY:-}" ]] && command -v curl >/dev/null 2>&1; then
  if me=$(curl -fsS \
      -H "Authorization: Bearer $POSTHOG_PERSONAL_API_KEY" \
      -H "Accept: application/json" \
      "${POSTHOG_HOST}/api/users/@me/" 2>/dev/null); then
    email=$(printf '%s' "$me" | jq -r '.email // .distinct_id // "unknown"')
    org=$(printf '%s' "$me"   | jq -r '.organization.name // .organization // "unknown"')
    org_id=$(printf '%s' "$me" | jq -r '.organization.id // ""')
    proj=$(printf '%s' "$me"  | jq -r '.team.name // (.team.api_token // "") // "unknown"')
    proj_id=$(printf '%s' "$me" | jq -r '.team.id // ""')
    ok "authenticated as $email"
    ok "  default org:     $org${org_id:+ ($org_id)}"
    ok "  default project: $proj${proj_id:+ (id $proj_id)}"
    if [[ -z "${POSTHOG_PROJECT_ID:-}" && -n "$proj_id" ]]; then
      warn "  Tip: export POSTHOG_PROJECT_ID=$proj_id to skip the arg in every command."
    fi
  else
    fail "POSTHOG_PERSONAL_API_KEY is set but auth failed against $POSTHOG_HOST/api/users/@me/."
    fail "  The key may be invalid, revoked, or for a different region (try POSTHOG_HOST=https://eu.posthog.com)."
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
