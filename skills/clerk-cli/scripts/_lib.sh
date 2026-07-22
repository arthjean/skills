# Shared bash helpers for clerk-cli scripts.
# Source from each script: source "$(dirname "$0")/_lib.sh"

set -euo pipefail

err() { printf '\033[31mERROR\033[0m: %s\n' "$1" >&2; exit 1; }

CLERK_API_BASE="${CLERK_API_BASE:-https://api.clerk.com/v1}"

# Auto-load CLERK_SECRET_KEY (and CLERK_API_VERSION) from a project .env file if not
# already set in the shell. Walks up from $PWD looking for .env.local then .env, stopping
# at the first git repo root or $HOME. Only Clerk-prefixed keys are extracted - the rest
# of the file is ignored (no shell evaluation, no var leakage).
#
# Precedence: shell env > .env.local > .env > error.
_clerk_load_env() {
  [[ -n "${CLERK_SECRET_KEY:-}" ]] && return 0

  local dir="${PWD}" parent
  while :; do
    for f in .env.local .env; do
      if [[ -f "$dir/$f" ]]; then
        local line key val
        while IFS= read -r line || [[ -n "$line" ]]; do
          [[ "$line" =~ ^[[:space:]]*# ]] && continue
          [[ "$line" =~ ^[[:space:]]*$ ]] && continue
          line="${line#export }"; line="${line# }"
          [[ "$line" != CLERK_SECRET_KEY=* && "$line" != CLERK_API_VERSION=* ]] && continue
          key="${line%%=*}"; val="${line#*=}"
          # Strip surrounding single or double quotes.
          [[ "$val" == \"*\" ]] && val="${val#\"}" && val="${val%\"}"
          [[ "$val" == \'*\' ]] && val="${val#\'}" && val="${val%\'}"
          case "$key" in
            CLERK_SECRET_KEY)
              [[ -z "${CLERK_SECRET_KEY:-}" ]] && export CLERK_SECRET_KEY="$val" \
                && export _CLERK_LOADED_FROM="$dir/$f"
              ;;
            CLERK_API_VERSION)
              [[ -z "${CLERK_API_VERSION:-}" ]] && export CLERK_API_VERSION="$val"
              ;;
          esac
        done < "$dir/$f"
        [[ -n "${CLERK_SECRET_KEY:-}" ]] && return 0
      fi
    done
    # Stop at repo root or once we'd traverse past $HOME / fs root.
    [[ -d "$dir/.git" ]] && return 0
    parent="$(dirname "$dir")"
    [[ "$parent" == "$dir" || "$dir" == "$HOME" ]] && return 0
    dir="$parent"
  done
}

# Try to load from .env first; this is a no-op if CLERK_SECRET_KEY is already exported.
_clerk_load_env

# Default to the current stable Backend API contract verified on 2026-07-16.
CLERK_API_VERSION="${CLERK_API_VERSION:-2026-05-12}"

# Require CLERK_SECRET_KEY in the environment (or loadable from a project .env file).
require_clerk_secret_key() {
  [[ -n "${CLERK_SECRET_KEY:-}" ]] || err \
"CLERK_SECRET_KEY is not set and no .env(.local) was found in the current directory or its parents.
Three ways to provide it (highest precedence first):
  1. export CLERK_SECRET_KEY=your_clerk_secret_key
  2. cd into a project with .env.local containing CLERK_SECRET_KEY=...
  3. run 'bunx clerk@latest env pull' inside a Clerk-linked project to write .env.local"
  case "$CLERK_SECRET_KEY" in
    sk_test_*|sk_live_*) ;;
    *) err "CLERK_SECRET_KEY format looks wrong (expected 'sk_test_...' or 'sk_live_...', got first 8 chars: '${CLERK_SECRET_KEY:0:8}...')." ;;
  esac
}

# Resolve a secret without placing its value in the process arguments.
# Accepted forms: @env:VARIABLE or - for one line from stdin.
read_secret_arg() {
  local spec="${1:-}" label="${2:-secret}" name value
  case "$spec" in
    @env:*)
      name="${spec#@env:}"
      [[ "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || err "invalid environment variable name for $label: $name"
      value="${!name:-}"
      [[ -n "$value" ]] || err "$label environment variable is empty: $name"
      ;;
    -)
      IFS= read -r value || err "failed to read $label from stdin"
      [[ -n "$value" ]] || err "$label read from stdin is empty"
      ;;
    *)
      err "refusing a literal $label in process arguments; use @env:VARIABLE or -"
      ;;
  esac
  printf '%s' "$value"
}

# URL-encode a single value for query strings.
# Usage: urlencode "user@example.com"
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

# Authenticated curl against api.clerk.com/v1, with one bounded 429 retry.
# Usage: clerk_api METHOD PATH [json_body | curl_extra_args...]
# Examples:
#   clerk_api GET    "/users?limit=20"
#   clerk_api POST   "/organizations" '{"name":"Acme","created_by":"user_xxx"}'
#   clerk_api PATCH  "/users/user_xxx" '{"first_name":"Alice"}'
#   clerk_api POST   "/sessions/sess_xxx/revoke"
clerk_api() {
  local method="${1:-GET}" path="${2:?path required}"
  shift 2

  # Path is expected to start with "/" - accept both /users and users.
  [[ "$path" == /* ]] || path="/$path"

  local args=(
    --silent --show-error
    --write-out '\n%{http_code}'
    -X "$method"
    -H "Authorization: Bearer $CLERK_SECRET_KEY"
    -H "Clerk-API-Version: $CLERK_API_VERSION"
    -H "Accept: application/json"
  )

  if [[ $# -gt 0 && ( "${1:0:1}" == "{" || "${1:0:1}" == "[" ) ]]; then
    args+=(-H "Content-Type: application/json" -d "$1")
    shift
  fi

  args+=("$@")

  local url="${CLERK_API_BASE}${path}"
  local attempt=1 max_attempts=2 response status body retry_after headers_file
  local retry_transport=false
  [[ "$method" == "GET" || "$method" == "HEAD" ]] && retry_transport=true
  headers_file=$(mktemp "${TMPDIR:-/tmp}/clerk-cli-headers.XXXXXX") \
    || err "failed to create a temporary headers file"

  while [[ $attempt -le $max_attempts ]]; do
    response=$(curl "${args[@]}" -D "$headers_file" "$url" 2>&1) || {
      if [[ "$retry_transport" == true && $attempt -lt $max_attempts ]]; then
        sleep 2
        attempt=$((attempt + 1))
        continue
      fi
      rm -f "$headers_file"
      if [[ "$retry_transport" == true ]]; then
        err "curl failed: $response"
      fi
      err "curl failed during ${method} ${path}; the remote outcome is unknown, so the request was not retried: $response"
    }

    # Last line is the status code; the preceding content is the body.
    status="${response##*$'\n'}"
    body="${response%$'\n'*}"

    if [[ "$status" == "429" ]]; then
      if [[ $attempt -ge $max_attempts ]]; then
        rm -f "$headers_file"
        err "API ${method} ${path} remained rate limited after ${max_attempts} attempts"
      fi
      retry_after=$(grep -i '^retry-after:' "$headers_file" 2>/dev/null | awk '{print $2}' | tr -d '\r' | head -n1)
      retry_after="${retry_after:-5}"
      [[ "$retry_after" =~ ^[0-9]+$ ]] || retry_after=5
      if (( retry_after > 30 )); then
        rm -f "$headers_file"
        err "API ${method} ${path} requested a ${retry_after}s rate-limit delay; stopped instead of blocking"
      fi
      printf '\033[33m!\033[0m 429 rate limited, sleeping %ss before retry\n' "$retry_after" >&2
      sleep "$retry_after"
      attempt=$((attempt + 1))
      continue
    fi

    rm -f "$headers_file"
    if [[ "$status" =~ ^2[0-9][0-9]$ ]]; then
      printf '%s' "$body"
      return 0
    fi
    err "API ${method} ${path} returned ${status}: $(printf '%s' "$body" | head -c 500)"
  done

  rm -f "$headers_file"
  err "API ${method} ${path} exhausted ${max_attempts} attempts"
}
