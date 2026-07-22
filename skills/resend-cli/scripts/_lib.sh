# Shared Bash helpers for the resend-cli skill.
# Source from each script: source "$(dirname "$0")/_lib.sh"

set -euo pipefail

err() { printf '\033[31mERROR\033[0m: %s\n' "$1" >&2; exit 1; }
warn() { printf '\033[33m!\033[0m %s\n' "$1" >&2; }

# Resend has a single global API. Override only for proxies, mocks, or self-hosted dev tunnels.
RESEND_HOST="${RESEND_HOST:-https://api.resend.com}"
RESEND_HOST="${RESEND_HOST%/}"
case "$RESEND_HOST" in
  https://*) ;;
  http://localhost|http://localhost:*|http://127.0.0.1|http://127.0.0.1:*)
    warn "RESEND_HOST uses local HTTP; never point a production key at an untrusted local service"
    ;;
  *) err "RESEND_HOST must use HTTPS, localhost, or 127.0.0.1" ;;
esac

# A non-empty User-Agent is REQUIRED - calls without it return error code 1010 / 403.
RESEND_USER_AGENT="${RESEND_USER_AGENT:-resend-agent-skill/1.0 (+https://resend.com/docs/api-reference)}"

# Auto-load RESEND_API_KEY and selected Resend values from a
# project .env file if not already set. Walks up from $PWD looking for .env.local then .env,
# stopping at the first git repo root or $HOME. Only RESEND_-prefixed keys are extracted:
# the rest of the file is ignored (no shell evaluation, no var leakage). RESEND_HOST and
# RESEND_USER_AGENT are intentionally never loaded from project files because they control
# where credentials are sent and how requests are identified.
#
# Precedence: shell env > .env.local > .env > error.
_resend_load_env() {
  [[ -n "${RESEND_API_KEY:-}" ]] && return 0

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
            RESEND_API_KEY=*|RESEND_KEY=*|RESEND_FROM=*|RESEND_AUDIENCE_ID=*|RESEND_WEBHOOK_SECRET=*) ;;
            *) continue ;;
          esac
          key="${line%%=*}"; val="${line#*=}"
          # Strip surrounding single or double quotes.
          [[ "$val" == \"*\" ]] && val="${val#\"}" && val="${val%\"}"
          [[ "$val" == \'*\' ]] && val="${val#\'}" && val="${val%\'}"
          case "$key" in
            RESEND_API_KEY|RESEND_KEY)
              if [[ -z "${RESEND_API_KEY:-}" ]]; then
                export RESEND_API_KEY="$val"
                export _RESEND_LOADED_FROM="$dir/$f"
              fi
              ;;
            RESEND_FROM)
              [[ -z "${RESEND_FROM:-}" ]] && export RESEND_FROM="$val"
              ;;
            RESEND_AUDIENCE_ID)
              [[ -z "${RESEND_AUDIENCE_ID:-}" ]] && export RESEND_AUDIENCE_ID="$val"
              ;;
            RESEND_WEBHOOK_SECRET)
              [[ -z "${RESEND_WEBHOOK_SECRET:-}" ]] && export RESEND_WEBHOOK_SECRET="$val"
              ;;
          esac
        done < "$dir/$f"
        [[ -n "${RESEND_API_KEY:-}" ]] && return 0
      fi
    done
    [[ -d "$dir/.git" ]] && return 0
    parent="$(dirname "$dir")"
    [[ "$parent" == "$dir" || "$dir" == "$HOME" ]] && return 0
    dir="$parent"
  done
}

# Try to load from .env first; no-op if RESEND_API_KEY is already exported.
_resend_load_env

# Require RESEND_API_KEY in the environment (or loadable from a project .env file).
require_resend_key() {
  [[ -n "${RESEND_API_KEY:-}" ]] || err \
"RESEND_API_KEY is not set and no .env(.local) was found in the current directory or its parents.
Three ways to provide it (highest precedence first):
  1. export RESEND_API_KEY=re_xxx
  2. cd into a project with .env.local containing RESEND_API_KEY=...
  3. Create one at Resend → Settings → API Keys → '+ Create API Key'
     Permission options: full_access (manage everything) or sending_access (send-only, optionally domain-scoped)"
  case "$RESEND_API_KEY" in
    re_*) ;;
    *) warn "RESEND_API_KEY does not start with 're_'. Continuing in case this is an intentional test stub." ;;
  esac
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

# Authenticated curl against $RESEND_HOST with 429 retry honoring Retry-After.
# Usage: resend_api METHOD PATH [json_body] [extra_curl_args...]
#
# To attach an Idempotency-Key (supported on POST /emails and POST /emails/batch only),
# set RESEND_IDEMPOTENCY_KEY in the calling script's env, e.g.:
#   RESEND_IDEMPOTENCY_KEY="signup-1234" resend_api POST /emails "$body"
#
# Examples:
#   resend_api GET    "/domains"
#   resend_api GET    "/emails?limit=20"
#   resend_api POST   "/emails" '{"from":"a@b.com","to":["c@d.com"],"subject":"hi","html":"<p>hi</p>"}'
#   resend_api PATCH  "/domains/abc-123" '{"name":"new-name"}'
#   resend_api DELETE "/api-keys/xyz"
resend_api() {
  local method="${1:-GET}" path="${2:?path required}"
  shift 2

  [[ "$path" == /* ]] || path="/$path"

  local request_body="" curl_config auth_value idempotency_value
  auth_value="${RESEND_API_KEY//\\/\\\\}"
  auth_value="${auth_value//\"/\\\"}"
  curl_config="header = \"Authorization: Bearer ${auth_value}\""$'\n'

  if [[ -n "${RESEND_IDEMPOTENCY_KEY:-}" ]]; then
    idempotency_value="${RESEND_IDEMPOTENCY_KEY//\\/\\\\}"
    idempotency_value="${idempotency_value//\"/\\\"}"
    curl_config+="header = \"Idempotency-Key: ${idempotency_value}\""$'\n'
  fi

  local args=(
    --silent --show-error
    --write-out '\n%{http_code}'
    -X "$method"
    -H "Accept: application/json"
    -H "User-Agent: $RESEND_USER_AGENT"
  )

  if [[ $# -gt 0 && ( "${1:0:1}" == "{" || "${1:0:1}" == "[" ) ]]; then
    request_body="$1"
    args+=(-H "Content-Type: application/json" --data-binary @-)
    shift
  fi

  args+=("$@")

  local url="${RESEND_HOST}${path}"
  local attempt=1 max_attempts=3 response status body retry_after
  local headers_file
  headers_file=$(mktemp "${TMPDIR:-/tmp}/resend-cli-headers.XXXXXX") || err "could not create a temporary headers file"

  while [[ $attempt -le $max_attempts ]]; do
    if [[ -n "$request_body" ]]; then
      response=$(printf '%s' "$request_body" \
        | curl --config /dev/fd/3 "${args[@]}" -D "$headers_file" "$url" 3<<<"$curl_config" 2>&1) || {
        [[ $attempt -lt $max_attempts ]] || { rm -f "$headers_file"; err "curl failed: $response"; }
        sleep 2; attempt=$((attempt + 1)); continue
      }
    else
      response=$(curl --config /dev/fd/3 "${args[@]}" -D "$headers_file" "$url" 3<<<"$curl_config" 2>&1) || {
      [[ $attempt -lt $max_attempts ]] || { rm -f "$headers_file"; err "curl failed: $response"; }
      sleep 2; attempt=$((attempt + 1)); continue
      }
    fi
    status="${response##*$'\n'}"
    body="${response%$'\n'*}"

    if [[ "$status" == "429" ]]; then
      retry_after=$(grep -i '^retry-after:' "$headers_file" 2>/dev/null | awk '{print $2}' | tr -d '\r' | head -n1)
      retry_after="${retry_after:-2}"
      # Clamp to avoid waiting for monthly/daily quota resets that span hours.
      if [[ "$retry_after" =~ ^[0-9]+$ ]] && [[ "$retry_after" -gt 60 ]]; then
        rm -f "$headers_file"
        err "429 rate limited with retry-after=${retry_after}s (>60s - likely daily/monthly quota). Body: $(printf '%s' "$body" | head -c 400)"
      fi
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

# Iterate every page of a paginated list endpoint, emitting one .data[] item per line as
# compact JSON. Resend uses cursor-based pagination: limit (1-100), after=<last_id>, before=<first_id>.
# Response shape: { "object": "list", "has_more": bool, "data": [...] }
#
# Usage: resend_paginate "/emails?limit=100"
#        resend_paginate "/contacts?limit=100&audience_id=$aud"
resend_paginate() {
  local path="${1:?path required}"
  local limit_default="${RESEND_PAGE_LIMIT:-100}"

  # Inject a default limit if the path lacks one (Resend default is 20).
  if [[ "$path" != *"limit="* ]]; then
    if [[ "$path" == *"?"* ]]; then path="${path}&limit=${limit_default}"
    else path="${path}?limit=${limit_default}"; fi
  fi

  local response has_more last_id sep
  while :; do
    response=$(resend_api GET "$path")
    printf '%s' "$response" | jq -c '.data[]?'
    has_more=$(printf '%s' "$response" | jq -r '.has_more // false')
    [[ "$has_more" != "true" ]] && return 0
    last_id=$(printf '%s' "$response" | jq -r '.data[-1].id // ""')
    [[ -z "$last_id" || "$last_id" == "null" ]] && return 0
    # Strip any existing after= param, then append the new cursor.
    if [[ "$path" == *"after="* ]]; then
      path=$(printf '%s' "$path" | sed -E 's/(after=)[^&]*/\1'"$last_id"'/')
    else
      sep='?' ; [[ "$path" == *"?"* ]] && sep='&'
      path="${path}${sep}after=${last_id}"
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
