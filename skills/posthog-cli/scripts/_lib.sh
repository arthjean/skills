# Shared bash helpers for posthog-cli scripts.
# Source from each script: source "$(dirname "$0")/_lib.sh"

set -euo pipefail

err() { printf '\033[31mERROR\033[0m: %s\n' "$1" >&2; exit 1; }
warn() { printf '\033[33m!\033[0m %s\n' "$1" >&2; }

_POSTHOG_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

# Match the official CLI variable names while preserving the legacy helper names.
POSTHOG_PERSONAL_API_KEY="${POSTHOG_PERSONAL_API_KEY:-${POSTHOG_CLI_API_KEY:-${POSTHOG_API_KEY:-}}}"
POSTHOG_PROJECT_ID="${POSTHOG_PROJECT_ID:-${POSTHOG_CLI_PROJECT_ID:-}}"
POSTHOG_HOST="${POSTHOG_HOST:-${POSTHOG_CLI_HOST:-https://us.posthog.com}}"
POSTHOG_HOST="${POSTHOG_HOST%/}"
export POSTHOG_PERSONAL_API_KEY POSTHOG_PROJECT_ID POSTHOG_HOST

# Auto-load POSTHOG_PERSONAL_API_KEY (and POSTHOG_PROJECT_ID, POSTHOG_HOST) from a project
# .env file if not already set in the shell. Walks up from $PWD looking for .env.local then
# .env, stopping at the first git repo root or $HOME. Only POSTHOG_-prefixed keys are
# extracted - the rest of the file is ignored (no shell evaluation, no var leakage).
#
# Precedence: shell env > .env.local > .env > error.
_posthog_load_env() {
  [[ -n "${POSTHOG_PERSONAL_API_KEY:-}" ]] && return 0

  local dir="${PWD}" parent
  while :; do
    for f in .env.local .env; do
      if [[ -f "$dir/$f" ]]; then
        local line key val
        while IFS= read -r line || [[ -n "$line" ]]; do
          [[ "$line" =~ ^[[:space:]]*# ]] && continue
          [[ "$line" =~ ^[[:space:]]*$ ]] && continue
          line="${line#export }"; line="${line# }"
          case "$line" in
            POSTHOG_PERSONAL_API_KEY=*|POSTHOG_API_KEY=*|POSTHOG_PROJECT_ID=*|POSTHOG_HOST=*|POSTHOG_ORG_ID=*|POSTHOG_CLI_API_KEY=*|POSTHOG_CLI_HOST=*|POSTHOG_CLI_PROJECT_ID=*) ;;
            *) continue ;;
          esac
          key="${line%%=*}"; val="${line#*=}"
          # Strip surrounding single or double quotes.
          [[ "$val" == \"*\" ]] && val="${val#\"}" && val="${val%\"}"
          [[ "$val" == \'*\' ]] && val="${val#\'}" && val="${val%\'}"
          case "$key" in
            POSTHOG_PERSONAL_API_KEY|POSTHOG_API_KEY|POSTHOG_CLI_API_KEY)
              if [[ -z "${POSTHOG_PERSONAL_API_KEY:-}" ]]; then
                export POSTHOG_PERSONAL_API_KEY="$val"
                export _POSTHOG_LOADED_FROM="$dir/$f"
              fi
              ;;
            POSTHOG_PROJECT_ID|POSTHOG_CLI_PROJECT_ID)
              [[ -z "${POSTHOG_PROJECT_ID:-}" ]] && export POSTHOG_PROJECT_ID="$val"
              ;;
            POSTHOG_HOST|POSTHOG_CLI_HOST)
              if [[ "$POSTHOG_HOST" == "https://us.posthog.com" ]]; then
                POSTHOG_HOST="${val%/}"
                export POSTHOG_HOST
              fi
              ;;
            POSTHOG_ORG_ID)
              [[ -z "${POSTHOG_ORG_ID:-}" ]] && export POSTHOG_ORG_ID="$val"
              ;;
          esac
        done < "$dir/$f"
        [[ -n "${POSTHOG_PERSONAL_API_KEY:-}" ]] && return 0
      fi
    done
    [[ -d "$dir/.git" ]] && return 0
    parent="$(dirname "$dir")"
    [[ "$parent" == "$dir" || "$dir" == "$HOME" ]] && return 0
    dir="$parent"
  done
}

# Try to load from .env first; this is a no-op if POSTHOG_PERSONAL_API_KEY is already exported.
_posthog_load_env

POSTHOG_HOST="${POSTHOG_HOST%/}"
[[ "$POSTHOG_HOST" =~ ^https?://[^/@?#]+$ ]] || err \
  "POSTHOG_HOST must be an HTTP(S) origin without a path, query, or fragment."

if [[ "$POSTHOG_HOST" == http://* ]] \
  && [[ ! "$POSTHOG_HOST" =~ ^http://(localhost|127\.0\.0\.1|\[::1\])(:[0-9]+)?$ ]] \
  && [[ "${POSTHOG_ALLOW_INSECURE_HTTP:-}" != "1" ]]; then
  err "Refusing to send a personal API key over plain HTTP. Use HTTPS or set POSTHOG_ALLOW_INSECURE_HTTP=1 for an explicitly trusted self-hosted instance."
fi

export POSTHOG_HOST

# Require POSTHOG_PERSONAL_API_KEY in the environment (or loadable from a project .env file).
require_posthog_key() {
  [[ -n "${POSTHOG_PERSONAL_API_KEY:-}" ]] || err \
"POSTHOG_PERSONAL_API_KEY is not set and no .env(.local) was found in the current directory or its parents.
Three ways to provide it (highest precedence first):
  1. export POSTHOG_PERSONAL_API_KEY=phx_xxx
  2. cd into a project with .env.local containing POSTHOG_PERSONAL_API_KEY=...
  3. Create one at <PostHog UI> → Settings → Personal API Keys → '+ Create personal API key'
     (max 10 keys per user; copy immediately - never shown again)"
  case "$POSTHOG_PERSONAL_API_KEY" in
    phx_*) ;;
    phc_*) err "POSTHOG_PERSONAL_API_KEY looks like a project API key (phc_*), not a personal key (phx_*). Project keys are public, ingestion-only - they cannot read/write the management API." ;;
    *) warn "POSTHOG_PERSONAL_API_KEY does not start with 'phx_' (got first 8 chars: '${POSTHOG_PERSONAL_API_KEY:0:8}...'). Continuing - older keys may use a different prefix." ;;
  esac
}

# Resolve project_id for project-scoped endpoints. Resolution order:
#   1. $1 (explicit arg passed to a script command)
#   2. POSTHOG_PROJECT_ID env var (or .env-loaded value)
#   3. fail with a helpful message
#
# Usage: pid=$(resolve_project_id "${1:-}")
resolve_project_id() {
  local arg="${1:-}"
  if [[ -n "$arg" ]]; then
    printf '%s' "$arg"
    return 0
  fi
  if [[ -n "${POSTHOG_PROJECT_ID:-}" ]]; then
    printf '%s' "$POSTHOG_PROJECT_ID"
    return 0
  fi
  err "POSTHOG_PROJECT_ID not set. Resolution order: explicit arg > env var. Find IDs with: bash \"$_POSTHOG_SCRIPT_DIR/posthog-projects.sh\" ls"
}

# URL-encode a single value for query strings. Usage: urlencode "user@example.com"
urlencode() {
  local s="${1:-}" out=""
  local i ch
  for ((i = 0; i < ${#s}; i++)); do
    ch="${s:i:1}"
    case "$ch" in
      [a-zA-Z0-9.~_-]) out+="$ch" ;;
      *) printf -v out '%s%%%02X' "$out" "'$ch" ;;
    esac
  done
  printf '%s' "$out"
}

# Authenticated curl against $POSTHOG_HOST, with built-in 429 retry honoring Retry-After.
# Usage: posthog_api METHOD PATH [json_body | curl_extra_args...]
# Examples:
#   posthog_api GET    "/api/projects/"
#   posthog_api GET    "/api/projects/123/feature_flags/?limit=50"
#   posthog_api POST   "/api/projects/123/feature_flags/" '{"key":"my-flag","name":"My flag","active":true}'
#   posthog_api PATCH  "/api/projects/123/feature_flags/456/" '{"active":false}'
#   posthog_api DELETE "/api/projects/123/cohorts/789/"
posthog_api() {
  local method="${1:-GET}" path="${2:?path required}"
  shift 2

  if [[ "$path" == http://* || "$path" == https://* ]]; then
    [[ "$path" == "$POSTHOG_HOST"/* ]] || err \
      "Refusing to send PostHog credentials to a different origin: $path"
    path="${path#"$POSTHOG_HOST"}"
  fi
  [[ "$path" == /* ]] || path="/$path"

  local args=(
    --silent --show-error
    --write-out '\n%{http_code}'
    -X "$method"
    -H "Authorization: Bearer $POSTHOG_PERSONAL_API_KEY"
    -H "Accept: application/json"
  )

  if [[ $# -gt 0 && "${1:0:1}" == "{" ]]; then
    args+=(-H "Content-Type: application/json" -d "$1")
    shift
  elif [[ $# -gt 0 && "${1:0:1}" == "[" ]]; then
    args+=(-H "Content-Type: application/json" -d "$1")
    shift
  fi

  args+=("$@")

  local url="${POSTHOG_HOST}${path}"
  local attempt=1 max_attempts=3 response status body retry_after
  local headers_file
  headers_file=$(mktemp "${TMPDIR:-/tmp}/posthog-cli-headers.XXXXXX")

  while [[ $attempt -le $max_attempts ]]; do
    response=$(curl "${args[@]}" -D "$headers_file" "$url" 2>&1) || {
      [[ $attempt -lt $max_attempts ]] || { rm -f "$headers_file"; err "curl failed: $response"; }
      sleep 2; attempt=$((attempt + 1)); continue
    }
    status="${response##*$'\n'}"
    body="${response%$'\n'*}"

    if [[ "$status" == "429" ]]; then
      retry_after=$(grep -i '^retry-after:' "$headers_file" 2>/dev/null | awk '{print $2}' | tr -d '\r' | head -n1)
      retry_after="${retry_after:-5}"
      printf '\033[33m!\033[0m 429 rate limited, sleeping %ss before retry (attempt %s/%s)\n' "$retry_after" "$attempt" "$max_attempts" >&2
      sleep "$retry_after"
      attempt=$((attempt + 1))
      continue
    fi

    rm -f "$headers_file"
    if [[ "$status" =~ ^2[0-9][0-9]$ ]]; then
      printf '%s' "$body"
      return 0
    fi
    err "API ${method} ${path} returned ${status}: $(printf '%s' "$body" | head -c 800)"
  done

  rm -f "$headers_file"
  err "API ${method} ${path} exhausted ${max_attempts} attempts"
}

# Iterate every page of a paginated list endpoint, emitting one .results[] item per line as
# compact JSON. Pass the initial path; the function follows .next URLs server-side.
# Usage: posthog_paginate "/api/projects/123/feature_flags/?limit=100"
posthog_paginate() {
  local path="${1:?path required}"
  local response next
  while [[ -n "$path" && "$path" != "null" ]]; do
    response=$(posthog_api GET "$path")
    printf '%s' "$response" | jq -c '.results[]?'
    next=$(printf '%s' "$response" | jq -r '.next // ""')
    if [[ -z "$next" || "$next" == "null" ]]; then
      return 0
    fi
    # Absolute pagination URLs must stay on the authenticated PostHog origin.
    if [[ "$next" == http* ]]; then
      [[ "$next" == "$POSTHOG_HOST"/* ]] || err \
        "Refusing cross-origin pagination URL: $next"
      path="${next#"$POSTHOG_HOST"}"
    else
      path="$next"
    fi
  done
}

# Pretty-print JSON if jq succeeds; otherwise print raw. Usage: pretty <<< "$json"
pretty() {
  local input
  input=$(cat)
  if [[ -z "$input" ]]; then
    return 0
  fi
  printf '%s' "$input" | jq . 2>/dev/null || printf '%s' "$input"
}
