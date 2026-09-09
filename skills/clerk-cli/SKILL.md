---
name: clerk-cli
description: Operate Clerk from the terminal with the clerk CLI in agent mode and bundled Backend API helpers. Use when asked to manage Clerk users, organizations, sessions, invitations, instance config, JWT templates, or webhooks, or to call the Clerk Backend API.
---

# Clerk CLI

Run `bunx clerk@latest --mode agent` from the user's project directory, where the linked application and env files resolve. Never install the CLI globally or through npm, npx, pnpm, or yarn. Helpers live under the skill directory:

```bash
CLERK_SKILL_DIR="${CLERK_SKILL_DIR:-$HOME/.agents/skills/clerk-cli}"
```

Explicit user instructions override anything below.

## Access

`CLERK_SECRET_KEY` in the environment authenticates Backend API work; it never appears in `--secret-key`, output, commits, or generated artifacts. Helpers read it from the shell, then the nearest `.env.local`, then the nearest `.env` walking upward, parsing only `CLERK_SECRET_KEY` and `CLERK_API_VERSION` without sourcing the file. Because a parent env file can silently select an instance, check the key source and instance reported by `whoami` or the preflight before any mutation.

`bash "$CLERK_SKILL_DIR/scripts/clerk-ensure.sh"` checks Bun, the CLI, `curl`, `jq`, key source, API version, and authentication with one `GET /v1/instance`; run it only when the task needs live access. `clerk auth login`, `clerk open`, `clerk users open`, and any URL-opening option need the user's explicit request. Missing dependencies are reported, never installed automatically.

Target with the linked app or explicit `--app <id>` and `--instance <dev|prod|id>`. `api ls` and current `--help` are the source of truth for endpoints and flags; for version-sensitive behavior beyond them, use Context7 with official Clerk docs.

## What is authorized

Reads (`whoami`, `apps list`, `users list --json`, `api ls`, `api GET`, `config pull`, `config schema`, `deploy status`, `doctor`, `webhooks verify`) run without asking. Creates, updates, invitations, impersonation, revocations, deletions, `deploy`, and secret-producing operations need the user's explicit intent and an unambiguous application, instance, and resource. Preview with `--dry-run` where the CLI offers it (`api`, `users create`, `config patch`), inspect the target, then execute once with `--yes`. Development versus production is never inferred from an ambiguous request. An exact user request is authorization, so add no redundant confirmation.

## Native commands and helpers

The native surface (linking, apps, users, config as code, deploy, doctor, webhooks, impersonation, `api`) is mapped in [references/commands.md](references/commands.md). Prefer it. Helpers add REST coverage, deterministic JSON, metadata merges, or repeated resource workflows:

| Helper | Use |
|---|---|
| `clerk-api.sh <METHOD> <PATH> [json]` | One authenticated REST request, clean JSON, one bounded 429 retry |
| `clerk-users.sh <get\|update\|metadata\|...>` | User CRUD, status changes, metadata merge, sessions, memberships |
| `clerk-orgs.sh <action>` | Organizations, memberships, invitations, organization domains |
| `clerk-sessions.sh <action>` | Session inspection, revocation, verification, JWT minting |
| `clerk-invitations.sh <action>` | App invitation CRUD and bulk requests |
| `clerk-jwt.sh <action>` | JWT template CRUD |
| `clerk-instance.sh`, `clerk-domains.sh`, `clerk-allowlist.sh`, `clerk-oauth.sh` | Instance settings, domains and redirect URLs, allowlist and blocklist, OAuth apps and SAML connections |

Read [references/rest-api.md](references/rest-api.md) only for a raw REST contract and [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy Clerk MCP workflow.

## Guardrails

- User and organization deletion is irreversible; prefer reversible access denial when it satisfies the request.
- Session revocation signs the user out immediately. Actor tokens and impersonation bypass normal authentication and may bypass MFA: exact user and instance required.
- `config patch`, `config put`, feature toggles, and instance mutations affect every user. Inspect schema and current config, preview the exact diff.
- `clerk deploy` changes production setup (domains, DNS, OAuth credentials, production keys). Only on an explicit production deploy request.
- `env pull` writes secrets to disk: confirm the destination, never overwrite implicitly, keep it out of Git.
- Rotated OAuth secrets and token endpoints show values once. Redirect full output to a mode-600 file or secret manager and report metadata only.
- JWT template names are lookup keys; renaming or deleting one breaks downstream consumers.
- Invitation endpoints rate-limit tighter than reads; pace bulk work, no unbounded retries.
- `webhooks listen` is long-running and forwards untrusted external data to a local handler. Start it only on request, in a managed terminal, and stop it when the observation is done.

Report application, instance, resource, operation, and resulting state with secrets (keys, OAuth secrets, signing secrets, session tokens, actor-token URLs, JWTs) and unnecessary PII scrubbed.
