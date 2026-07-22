# Shared bash helpers for vercel-cli scripts.
# Source from each script: source "$(dirname "$0")/_lib.sh"

set -euo pipefail

err() { printf '\033[31mERROR\033[0m: %s\n' "$1" >&2; exit 1; }

# Require VERCEL_TOKEN in the environment.
require_vercel_token() {
  [[ -n "${VERCEL_TOKEN:-}" ]] || err \
    "VERCEL_TOKEN is not set. Generate at https://vercel.com/account/tokens and: export VERCEL_TOKEN=vcp_xxx"
}

# Resolve team ID/slug from arg, VERCEL_TEAM_ID, VERCEL_ORG_ID, or .vercel/project.json. Echoes the ID (or empty if no team).
# Empty result is valid - personal accounts have no teamId.
resolve_team_id() {
  local override="${1:-}"
  if [[ -n "$override" ]]; then
    echo "$override"; return 0
  fi
  if [[ -n "${VERCEL_TEAM_ID:-}" ]]; then
    echo "$VERCEL_TEAM_ID"; return 0
  fi
  if [[ -n "${VERCEL_ORG_ID:-}" ]]; then
    echo "$VERCEL_ORG_ID"; return 0
  fi
  if [[ -f .vercel/project.json ]]; then
    local tid
    tid=$(jq -r '.orgId // empty' .vercel/project.json 2>/dev/null || true)
    [[ -n "$tid" ]] && { echo "$tid"; return 0; }
  fi
  # Personal account is a valid case - return empty.
  echo ""
}

# Resolve project ID from arg, VERCEL_PROJECT_ID, or .vercel/project.json. Echoes the ID.
resolve_project_id() {
  local override="${1:-}"
  if [[ -n "$override" ]]; then
    echo "$override"; return 0
  fi
  if [[ -n "${VERCEL_PROJECT_ID:-}" ]]; then
    echo "$VERCEL_PROJECT_ID"; return 0
  fi
  if [[ -f .vercel/project.json ]]; then
    local pid
    pid=$(jq -r '.projectId // empty' .vercel/project.json 2>/dev/null || true)
    [[ -n "$pid" ]] && { echo "$pid"; return 0; }
  fi
  err "no project ID. Pass via env (VERCEL_PROJECT_ID=...), run 'bunx vercel@latest link --yes' inside the repo, or pass an explicit project argument."
}

# Append ?teamId=<id> to a path if a team is resolvable. Echoes the path with query string.
# Args: <path> [team_override]
with_team_query() {
  local path="${1:?path required}"
  local override="${2:-}"
  local team
  team=$(resolve_team_id "$override")
  if [[ -n "$team" ]]; then
    if [[ "$path" == *"?"* ]]; then
      echo "${path}&teamId=${team}"
    else
      echo "${path}?teamId=${team}"
    fi
  else
    echo "$path"
  fi
}

# Authenticated curl against api.vercel.com.
# Usage: vercel_api METHOD PATH [json_body | curl_extra_args...]
# Examples:
#   vercel_api GET "/v9/projects"
#   vercel_api POST "/v1/webhooks" '{"url":"https://...","events":["deployment.succeeded"]}'
#   vercel_api DELETE "/v1/webhooks/wh_xxx"
vercel_api() {
  local method="${1:-GET}" path="${2:?path required}"
  shift 2

  local body=""
  local auth_config
  auth_config=$(printf 'header = "Authorization: Bearer %s"\n' "$VERCEL_TOKEN")

  local args=(
    -fsS
    --retry 1 --retry-delay 5 --retry-connrefused
    -X "$method"
    -H "Accept: application/json"
  )

  # If the next arg looks like a JSON body, attach it.
  if [[ $# -gt 0 && ( "${1:0:1}" == "{" || "${1:0:1}" == "[" ) ]]; then
    body="$1"
    args+=(-H "Content-Type: application/json" --data-binary @-)
    shift
  fi

  # Forward any remaining flags (e.g. --data-urlencode).
  args+=("$@")

  if [[ -n "$body" ]]; then
    printf '%s' "$body" \
      | curl --config /dev/fd/3 "${args[@]}" "https://api.vercel.com${path}" 3<<<"$auth_config"
  else
    curl --config /dev/fd/3 "${args[@]}" "https://api.vercel.com${path}" 3<<<"$auth_config"
  fi
}
