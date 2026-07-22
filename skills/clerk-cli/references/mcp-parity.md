# Legacy Clerk MCP parity map

Use this reference only when translating a legacy Clerk MCP management workflow to the current CLI or bundled helpers. Do not assume the current MCP server still exposes the same names. If the task is about configuring or inspecting a live MCP server, use its current tool list instead.

The mappings below come from the historical `@clerk/agent-toolkit` management surface. For Clerk SDK documentation and code snippets, use Context7 with the official Clerk docs rather than this operational skill.

## Users

| Legacy MCP tool | Codex CLI equivalent | Notes |
|---|---|---|
| `getUser` | `bunx clerk@latest --mode agent api /users/<user_id>` | Read-only |
| `getUserList` | `bunx clerk@latest --mode agent users list --json [filters]` | Native filters and pagination |
| `getUserCount` | `bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" count` | Returns `total_count` |
| `createUser` | `bunx clerk@latest --mode agent users create ... --dry-run` | Preview before `--yes` |
| `updateUser` | `bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" update <id> '<patch-json>'` | General attributes |
| `updateUser` metadata | `bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" metadata <id> <public|private|unsafe> '<merge-json>'` | Atomic deep merge through the 2026-05-12 metadata endpoint |
| `deleteUser` | `bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" rm <user_id>` | Irreversible |

## Organizations

| Legacy MCP tool | Codex CLI equivalent |
|---|---|
| `getOrganization` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" get <org_id>` |
| `getOrganizationList` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" ls [limit] [offset]` |
| `createOrganization` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" create <name> <created_by_user_id> [slug]` |
| `updateOrganization` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" update <org_id> '<patch-json>'` |
| `updateOrganization` metadata | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" metadata <org_id> <public|private> '<merge-json>'` |
| `deleteOrganization` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" rm <org_id>` |

## Memberships

| Legacy MCP tool | Codex CLI equivalent |
|---|---|
| `getOrganizationMembershipList` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" members <org_id> [limit] [offset]` |
| `createOrganizationMembership` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" add-member <org_id> <user_id> <role>` |
| `updateOrganizationMembership` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" update-role <org_id> <user_id> <role>` |
| `deleteOrganizationMembership` | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" rm-member <org_id> <user_id>` |

## Invitations

| Legacy MCP tool | Codex CLI equivalent |
|---|---|
| `createInvitation` | `bash "$CLERK_SKILL_DIR/scripts/clerk-invitations.sh" create <email> [redirect] [metadata]` |
| `getInvitationList` | `bash "$CLERK_SKILL_DIR/scripts/clerk-invitations.sh" ls [status] [limit] [offset]` |
| `revokeInvitation` | `bash "$CLERK_SKILL_DIR/scripts/clerk-invitations.sh" revoke <invitation_id>` |
| Bulk gap | `bash "$CLERK_SKILL_DIR/scripts/clerk-invitations.sh" bulk <emails-file> [redirect]` |

## Additional Backend API coverage

| Resource | Codex CLI entrypoint |
|---|---|
| Sessions | `bash "$CLERK_SKILL_DIR/scripts/clerk-sessions.sh" <action> ...` |
| JWT templates | `bash "$CLERK_SKILL_DIR/scripts/clerk-jwt.sh" <action> ...` |
| Instance settings | `bash "$CLERK_SKILL_DIR/scripts/clerk-instance.sh" <action> ...` |
| Allowlist and blocklist | `bash "$CLERK_SKILL_DIR/scripts/clerk-allowlist.sh" <action> ...` |
| Domains and redirect URLs | `bash "$CLERK_SKILL_DIR/scripts/clerk-domains.sh" <action> ...` |
| OAuth applications and SAML | `bash "$CLERK_SKILL_DIR/scripts/clerk-oauth.sh" <action> ...` |
| Sign-in, actor, and testing tokens | `bash "$CLERK_SKILL_DIR/scripts/clerk-oauth.sh" <signin-token|actor-token|testing-token> ...` |
| Generic Backend API call | `bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" <METHOD> <PATH> [body]` |
| Current endpoint discovery | `bunx clerk@latest --mode agent api ls [filter]` |

## Translation workflow

1. Identify the MCP tool's resource, action, and arguments.
2. Resolve the exact application and instance with `whoami` or explicit IDs.
3. Prefer a current native CLI command when one exists.
4. Otherwise choose the narrowest helper in the tables above.
5. Preserve read versus mutation semantics. Do not translate a read request into a mutation or broaden a single-resource request into a bulk action.
6. Inspect destructive targets and preview supported mutations before using `--yes`.
7. Report the resulting resource state without exposing keys, tokens, sign-in URLs, or unnecessary PII.

Example: promote a known user to organization admin.

```bash
bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" \
  update-role org_xxx user_yyy org:admin
```

Example: revoke all active sessions for one inspected user.

```bash
bash "$CLERK_SKILL_DIR/scripts/clerk-sessions.sh" \
  revoke-user-sessions user_xxx
```

Both operations mutate live authentication state. An exact user request is authorization; otherwise stop when the requested scope is ambiguous.
