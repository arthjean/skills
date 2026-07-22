---
name: clerk-cli
description: "Operate Clerk from a coding-agent terminal with the official clerk CLI and bundled Backend API helpers. Use when the agent needs to inspect or manage Clerk applications, users, organizations, memberships, invitations, sessions, impersonation, JWT templates, instance configuration, allowlists, blocklists, domains, OAuth applications, SAML connections, webhooks, or Clerk tokens; invoke clerk-cli or the Clerk CLI; diagnose Clerk authentication or project linking; call the Clerk Backend API; or translate a legacy Clerk MCP management workflow to shell commands. Do not use for application runtime integration through Clerk SDKs, frontend sign-in flows, continuous monitoring, or Clerk MCP server configuration."
---

# Clerk CLI

Operate Clerk through `bunx clerk@latest --mode agent` and the bundled Bash helpers. Keep the working directory in the user's project so Clerk can resolve its linked application and local environment files.

## Operating contract

1. Set the absolute directory containing this `SKILL.md` as `CLERK_SKILL_DIR`. In a standard user installation:

   ```bash
   CLERK_SKILL_DIR="${CLERK_SKILL_DIR:-$HOME/.agents/skills/clerk-cli}"
   ```

2. Do not `cd` into the skill directory. Invoke helpers with `bash "$CLERK_SKILL_DIR/scripts/<script>.sh"` from the target project.
3. Use `bunx clerk@latest --mode agent`. Do not install Clerk CLI globally and do not use npm, npx, pnpm, or yarn.
4. Prefer `CLERK_SECRET_KEY` in the environment for Backend API operations. Never place a literal key in `--secret-key`, command output, committed files, or generated artifacts.
5. Prefer the official CLI. Use bundled helpers for REST gaps, deterministic JSON pipelines, metadata merges, or repeated resource workflows.
6. Keep browser use opt-in. Do not run `clerk auth login`, `clerk open`, `clerk users open`, or an option that opens a URL unless the user explicitly requested browser authentication or navigation.
7. Require explicit intent and an unambiguous application, instance, and resource before any create, update, invitation, impersonation, revocation, deletion, deploy, or secret-producing operation. An exact user request is authorization, so do not add a redundant confirmation.
8. Inspect the active application, instance type, and target resource before a destructive or production-wide mutation. Do not infer development versus production from an ambiguous request.
9. Treat secret keys, OAuth client secrets, webhook signing secrets, session tokens, actor-token URLs, JWTs, and user PII as sensitive. Scrub them from summaries and avoid retrieving them when metadata is sufficient.
10. Do not install missing dependencies automatically. Report the missing executable and an OS-appropriate install command after detecting the host environment.

## Preflight

Run the bundled preflight only when the task needs live Clerk access:

```bash
bash "$CLERK_SKILL_DIR/scripts/clerk-ensure.sh"
```

It checks Bun, the current Clerk CLI, `curl`, `jq`, the secret-key source, API version, and authentication with one read-only `GET /v1/instance` request.

For documentation-only work, skip live authentication and inspect current CLI help:

```bash
bunx clerk@latest --mode agent --help
bunx clerk@latest --mode agent <command> --help
```

For version-sensitive behavior not covered by current help, use Context7 with official Clerk documentation. The current CLI help and `clerk api ls` override examples bundled in this skill.

## Authentication and targeting

The official CLI can target the application linked to the current project or an explicit `--app <id>` and `--instance <dev|prod|instance_id>`. Before a mutation, prefer explicit targeting or verify the linked target with:

```bash
bunx clerk@latest --mode agent whoami
```

Use an environment key for headless Backend API access:

```bash
export CLERK_SECRET_KEY=your_clerk_secret_key
bunx clerk@latest --mode agent api /users
```

Bundled helpers resolve `CLERK_SECRET_KEY` in this order:

1. Existing shell environment
2. Nearest `.env.local` while walking upward from the current directory
3. Nearest `.env` while walking upward from the current directory

Only `CLERK_SECRET_KEY` and `CLERK_API_VERSION` are parsed. The files are never sourced. Before a mutation, check the preflight's reported key source and instance environment so an unrelated parent env file cannot silently select the target.

`clerk auth login` uses browser OAuth and stored credentials. Use it only when the user explicitly wants interactive account authentication. For direct Backend API work, an environment key is sufficient.

## Execution workflow

1. Classify the request as read-only, mutating, destructive, production-wide, browser-opening, long-running, or secret-producing.
2. Resolve the active application and instance with `whoami`, explicit IDs, or a read-only instance request.
3. Inspect the exact target before mutation. Use `--dry-run` for `api`, user creation, and config changes when available.
4. Prefer a native command from [references/commands.md](references/commands.md). Use a bundled helper only when it adds required REST coverage or deterministic behavior.
5. Execute the narrowest authorized operation. Use `--yes` only after the target and payload have been inspected.
6. Inspect structured output and report the application, instance, resource, operation, and resulting state without exposing secrets or unnecessary PII.

## Quick map

| Intent | Command |
|---|---|
| Verify account and linked application | `bunx clerk@latest --mode agent whoami` |
| List applications | `bunx clerk@latest --mode agent apps list` |
| Link the current project | `bunx clerk@latest --mode agent link --app <app_id>` |
| List or search users | `bunx clerk@latest --mode agent users list --json [filters]` |
| Preview user creation | `bunx clerk@latest --mode agent users create --email <email> --dry-run` |
| Create an inspected user | `bunx clerk@latest --mode agent users create --email <email> --yes --json` |
| Discover Backend API endpoints | `bunx clerk@latest --mode agent api ls [filter]` |
| Preview a raw mutation | `bunx clerk@latest --mode agent api <path> -X PATCH -d '<json>' --dry-run` |
| Pull current instance config | `bunx clerk@latest --mode agent config pull` |
| Inspect config schema | `bunx clerk@latest --mode agent config schema --keys <section>` |
| Preview a config patch | `bunx clerk@latest --mode agent config patch --json '<json>' --dry-run` |
| Check production deploy state | `bunx clerk@latest --mode agent deploy status` |
| Diagnose the current project | `bunx clerk@latest --mode agent doctor` |
| Verify live helper context | `bash "$CLERK_SKILL_DIR/scripts/clerk-ensure.sh"` |
| Get or update a user through REST | `bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" <get|update|metadata> ...` |
| Manage organizations and memberships | `bash "$CLERK_SKILL_DIR/scripts/clerk-orgs.sh" <action> ...` |
| Inspect or revoke sessions | `bash "$CLERK_SKILL_DIR/scripts/clerk-sessions.sh" <action> ...` |
| Manage app invitations | `bash "$CLERK_SKILL_DIR/scripts/clerk-invitations.sh" <action> ...` |
| Manage JWT templates | `bash "$CLERK_SKILL_DIR/scripts/clerk-jwt.sh" <action> ...` |
| Manage allowlist or blocklist identifiers | `bash "$CLERK_SKILL_DIR/scripts/clerk-allowlist.sh" <action> ...` |
| Call a raw Backend API path | `bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" <METHOD> <PATH> [json-body]` |
| Verify a webhook delivery offline | `bunx clerk@latest --mode agent webhooks verify ...` |

## Native CLI and helper boundary

Use the native CLI for project linking, application management, user listing and creation, configuration as code, deploy status, diagnostics, webhook tools, impersonation, and ad hoc API requests. Its agent mode provides deterministic, non-interactive output and its current endpoint catalog is the source of truth:

```bash
bunx clerk@latest --mode agent api ls
bunx clerk@latest --mode agent api /users/user_xxx
```

Use the helpers when they provide a narrower interface:

- `clerk-api.sh`: one authenticated REST request with clean JSON and one bounded 429 retry
- `clerk-users.sh`: user CRUD, status changes, metadata merge, sessions, and memberships
- `clerk-orgs.sh`: organization, membership, invitation, and organization-domain workflows
- `clerk-sessions.sh`: session inspection, revocation, verification, and JWT minting
- `clerk-invitations.sh`: app invitation CRUD and bulk requests
- `clerk-jwt.sh`: JWT template CRUD
- `clerk-instance.sh`: raw Backend API instance settings
- `clerk-allowlist.sh`: allowlist and blocklist identifiers
- `clerk-domains.sh`: instance domains and redirect URLs
- `clerk-oauth.sh`: OAuth applications, SAML connections, and token endpoints

Read [references/rest-api.md](references/rest-api.md) only for a raw REST contract. Read [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy Clerk MCP management workflow.

## Guardrails

- User and organization deletion is irreversible. Prefer reversible access denial when it satisfies the request.
- Session revocation immediately signs a user out. Actor tokens and impersonation bypass normal user authentication and may bypass MFA. Require an exact user and instance.
- `config patch`, `config put`, feature enable or disable, and instance helper mutations can affect every user. Inspect schema and current config, then preview the exact diff.
- `clerk deploy` changes production setup and can involve domains, DNS, OAuth credentials, and production keys. Run only when the user explicitly requests a production deploy.
- `env pull` writes secret-bearing files. Confirm the destination, do not overwrite an existing file implicitly, and keep the result out of Git.
- OAuth secret rotation and secret-producing token endpoints return values that may be shown once. Redirect the full output to an approved mode-600 file or secret manager and report only non-secret metadata.
- JWT template names are lookup keys. Changing or deleting one can break downstream token consumers.
- Invitation endpoints have tighter rate limits than ordinary reads. Pace bulk workflows and never use an unbounded retry loop.
- `webhooks listen` is long-running and forwards untrusted external event data to a local handler. Start it only when requested, use a managed terminal session, and stop it when the requested observation is complete.
- For any flag or endpoint not documented here, inspect current `--help` or `api ls` rather than guessing.

## References

- Read [references/commands.md](references/commands.md) for the verified official CLI surface and non-interactive patterns.
- Read [references/rest-api.md](references/rest-api.md) for the pinned Backend API helper contract and endpoint catalog.
- Read [references/mcp-parity.md](references/mcp-parity.md) for legacy MCP-to-CLI translations.
