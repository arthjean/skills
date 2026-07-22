#!/usr/bin/env bash
# clerk-orgs.sh - full Organization + Membership + Invitation + Domain CRUD.
# Replaces @clerk/agent-toolkit MCP tools: getOrganization, getOrganizationList,
# createOrganization, updateOrganization, deleteOrganization,
# getOrganizationMembershipList, createOrganizationMembership,
# updateOrganizationMembership, deleteOrganizationMembership.
#
# Usage:
#   ./clerk-orgs.sh ls               [limit] [offset]
#   ./clerk-orgs.sh get              <org_id>
#   ./clerk-orgs.sh create           <name> <created_by_user_id> [slug]
#   ./clerk-orgs.sh update           <org_id> <patch-json>
#   ./clerk-orgs.sh metadata         <org_id> {public|private} <merge-json>
#   ./clerk-orgs.sh members          <org_id> [limit] [offset]
#   ./clerk-orgs.sh add-member       <org_id> <user_id> <role>
#   ./clerk-orgs.sh update-role      <org_id> <user_id> <new_role>
#   ./clerk-orgs.sh rm-member        <org_id> <user_id>
#   ./clerk-orgs.sh invitations      <org_id> [status]
#   ./clerk-orgs.sh invite           <org_id> <email> <role> <inviter_user_id> [redirect_url]
#   ./clerk-orgs.sh revoke-invite    <org_id> <invitation_id> <requesting_user_id>
#   ./clerk-orgs.sh domains          <org_id>
#   ./clerk-orgs.sh add-domain       <org_id> <domain> [enrollment_mode]
#   ./clerk-orgs.sh rm-domain        <org_id> <domain_id>
#   ./clerk-orgs.sh rm               <org_id>
#
# Roles: typically "org:admin", "org:member" (or your custom roles).
# Enrollment modes: manual_invitation | automatic_invitation | automatic_suggestion.

source "$(dirname "$0")/_lib.sh"
require_clerk_secret_key

[[ $# -ge 1 ]] || err "usage: $0 <action> [args...]"
action="$1"; shift

case "$action" in
  ls)
    limit="${1:-20}"; offset="${2:-0}"
    clerk_api GET "/organizations?limit=${limit}&offset=${offset}" | jq .
    ;;

  get)
    [[ $# -ge 1 ]] || err "usage: $0 get <org_id>"
    clerk_api GET "/organizations/$1" | jq .
    ;;

  create)
    [[ $# -ge 2 ]] || err "usage: $0 create <name> <created_by_user_id> [slug]"
    name="$1"; creator="$2"; slug="${3:-}"
    body=$(jq -nc --arg name "$name" --arg by "$creator" --arg slug "$slug" \
      '{name: $name, created_by: $by} + (if $slug != "" then {slug: $slug} else {} end)')
    clerk_api POST "/organizations" "$body" | jq .
    ;;

  update)
    [[ $# -ge 2 ]] || err "usage: $0 update <org_id> <patch-json>"
    clerk_api PATCH "/organizations/$1" "$2" | jq .
    ;;

  metadata)
    [[ $# -ge 3 ]] || err "usage: $0 metadata <org_id> {public|private} <merge-json>"
    oid="$1"; kind="$2"; patch="$3"
    case "$kind" in public|private) ;; *) err "kind must be: public | private" ;; esac
    # API version 2026-05-12 provides an atomic deep-merge endpoint.
    body=$(jq -nc --argjson m "$patch" --arg k "${kind}_metadata" '{($k): $m}')
    clerk_api PATCH "/organizations/$oid/metadata" "$body" | jq .
    ;;

  members)
    [[ $# -ge 1 ]] || err "usage: $0 members <org_id> [limit] [offset]"
    oid="$1"; limit="${2:-20}"; offset="${3:-0}"
    clerk_api GET "/organizations/${oid}/memberships?limit=${limit}&offset=${offset}" | jq .
    ;;

  add-member)
    [[ $# -ge 3 ]] || err "usage: $0 add-member <org_id> <user_id> <role>"
    body=$(jq -nc --arg uid "$2" --arg role "$3" '{user_id: $uid, role: $role}')
    clerk_api POST "/organizations/$1/memberships" "$body" | jq .
    ;;

  update-role)
    [[ $# -ge 3 ]] || err "usage: $0 update-role <org_id> <user_id> <new_role>"
    body=$(jq -nc --arg role "$3" '{role: $role}')
    clerk_api PATCH "/organizations/$1/memberships/$2" "$body" | jq .
    ;;

  rm-member)
    [[ $# -ge 2 ]] || err "usage: $0 rm-member <org_id> <user_id>"
    clerk_api DELETE "/organizations/$1/memberships/$2" | jq .
    ;;

  invitations)
    [[ $# -ge 1 ]] || err "usage: $0 invitations <org_id> [status]"
    oid="$1"; status="${2:-}"
    if [[ -n "$status" ]]; then
      clerk_api GET "/organizations/${oid}/invitations?status=${status}" | jq .
    else
      clerk_api GET "/organizations/${oid}/invitations" | jq .
    fi
    ;;

  invite)
    [[ $# -ge 4 ]] || err "usage: $0 invite <org_id> <email> <role> <inviter_user_id> [redirect_url]"
    body=$(jq -nc \
      --arg email "$2" --arg role "$3" --arg inviter "$4" --arg redir "${5:-}" \
      '{email_address: $email, role: $role, inviter_user_id: $inviter}
       + (if $redir != "" then {redirect_url: $redir} else {} end)')
    clerk_api POST "/organizations/$1/invitations" "$body" | jq .
    ;;

  revoke-invite)
    [[ $# -ge 3 ]] || err "usage: $0 revoke-invite <org_id> <invitation_id> <requesting_user_id>"
    body=$(jq -nc --arg ruid "$3" '{requesting_user_id: $ruid}')
    clerk_api POST "/organizations/$1/invitations/$2/revoke" "$body" | jq .
    ;;

  domains)
    [[ $# -ge 1 ]] || err "usage: $0 domains <org_id>"
    clerk_api GET "/organizations/$1/domains" | jq .
    ;;

  add-domain)
    [[ $# -ge 2 ]] || err "usage: $0 add-domain <org_id> <domain> [enrollment_mode]"
    mode="${3:-manual_invitation}"
    body=$(jq -nc --arg d "$2" --arg m "$mode" '{name: $d, enrollment_mode: $m}')
    clerk_api POST "/organizations/$1/domains" "$body" | jq .
    ;;

  rm-domain)
    [[ $# -ge 2 ]] || err "usage: $0 rm-domain <org_id> <domain_id>"
    clerk_api DELETE "/organizations/$1/domains/$2" | jq .
    ;;

  rm)
    [[ $# -ge 1 ]] || err "usage: $0 rm <org_id>"
    clerk_api DELETE "/organizations/$1" | jq .
    ;;

  *)
    err "unknown action: $action"
    ;;
esac
