#!/usr/bin/env bash
# posthog-projects.sh - list / get / create / update PostHog projects (a.k.a. teams).
# Replaces MCP tools: project-get, projects-get, switch-project (+ org listing helpers).
#
# Usage:
#   ./posthog-projects.sh ls                                  # all projects across all orgs you can access
#   ./posthog-projects.sh ls-org <org_id>                     # projects in a single organization
#   ./posthog-projects.sh get [project_id]                    # default: $POSTHOG_PROJECT_ID
#   ./posthog-projects.sh create <org_id> <name>              # creates a new project in the org
#   ./posthog-projects.sh update <project_id> <patch-json>
#   ./posthog-projects.sh switch <project_id>                 # prints the export line; eval to apply
#   ./posthog-projects.sh me                                  # current user + default org/project (cheap)

source "$(dirname "$0")/_lib.sh"
require_posthog_key
SCRIPT_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"

[[ $# -ge 1 ]] || err "usage: $0 {ls|ls-org|get|create|update|switch|me} [args...]"

action="$1"; shift

case "$action" in
  ls)
    posthog_api GET "/api/projects/" | pretty
    ;;

  ls-org)
    [[ $# -ge 1 ]] || err "usage: $0 ls-org <org_id>"
    posthog_api GET "/api/organizations/$1/projects/" | pretty
    ;;

  get)
    pid=$(resolve_project_id "${1:-}")
    posthog_api GET "/api/projects/$pid/" | pretty
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <org_id> <name>"
    org="$1"; name="$2"
    body=$(jq -nc --arg name "$name" '{name: $name}')
    posthog_api POST "/api/organizations/$org/projects/" "$body" | pretty
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <project_id> <patch-json>"
    posthog_api PATCH "/api/projects/$1/" "$2" | pretty
    ;;

  switch)
    [[ $# -ge 1 ]] || err "usage: $0 switch <project_id>"
    pid="$1"
    # Verify the ID is reachable before printing the export hint.
    posthog_api GET "/api/projects/$pid/" >/dev/null
    printf 'export POSTHOG_PROJECT_ID=%s\n' "$pid"
    printf '\n# Run: eval "$(bash %q switch %q)"\n' "$SCRIPT_DIR/posthog-projects.sh" "$pid" >&2
    ;;

  me)
    posthog_api GET "/api/users/@me/" | pretty
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
