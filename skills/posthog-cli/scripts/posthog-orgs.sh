#!/usr/bin/env bash
# posthog-orgs.sh - organizations, members, roles.
# Replaces MCP tools: organizations-list, organization-get, org-members-list, switch-organization,
#                     roles-list, role-get, role-members-list.
#
# Usage:
#   ./posthog-orgs.sh ls                          # all orgs you can access
#   ./posthog-orgs.sh get <org_id>
#   ./posthog-orgs.sh members <org_id>
#   ./posthog-orgs.sh rm-member <org_id> <member_id>
#   ./posthog-orgs.sh roles <org_id>              # custom roles
#   ./posthog-orgs.sh role <org_id> <role_id>
#   ./posthog-orgs.sh role-members <org_id> <role_id>
#   ./posthog-orgs.sh activity <org_id>           # org-level activity log
#   ./posthog-orgs.sh switch <org_id>             # print export hint

source "$(dirname "$0")/_lib.sh"
require_posthog_key

[[ $# -ge 1 ]] || err "usage: $0 {ls|get|members|rm-member|roles|role|role-members|activity|switch} [args...]"

action="$1"; shift

case "$action" in
  ls)
    posthog_api GET "/api/organizations/" | pretty
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <org_id>"
    posthog_api GET "/api/organizations/$1/" | pretty
    ;;

  members)
    [[ $# -ge 1 ]] || err "usage: $0 members <org_id>"
    posthog_api GET "/api/organizations/$1/members/" | pretty
    ;;

  rm-member)
    [[ $# -ge 2 ]] || err "usage: $0 rm-member <org_id> <member_id>"
    posthog_api DELETE "/api/organizations/$1/members/$2/" | pretty
    ;;

  roles)
    [[ $# -ge 1 ]] || err "usage: $0 roles <org_id>"
    posthog_api GET "/api/organizations/$1/roles/" | pretty
    ;;

  role)
    [[ $# -ge 2 ]] || err "usage: $0 role <org_id> <role_id>"
    posthog_api GET "/api/organizations/$1/roles/$2/" | pretty
    ;;

  role-members)
    [[ $# -ge 2 ]] || err "usage: $0 role-members <org_id> <role_id>"
    posthog_api GET "/api/organizations/$1/roles/$2/role_memberships/" | pretty
    ;;

  activity)
    [[ $# -ge 1 ]] || err "usage: $0 activity <org_id>"
    posthog_api GET "/api/organizations/$1/activity_log/" | pretty
    ;;

  switch)
    [[ $# -ge 1 ]] || err "usage: $0 switch <org_id>"
    posthog_api GET "/api/organizations/$1/" >/dev/null
    printf 'export POSTHOG_ORG_ID=%s\n' "$1"
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
