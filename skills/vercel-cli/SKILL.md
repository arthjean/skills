---
name: vercel-cli
description: Operate Vercel from the terminal with the Vercel CLI and bundled REST helpers. Use when asked to deploy, inspect deployments or logs, or manage Vercel projects, env vars, domains, or Edge Config.
---

# Vercel CLI

Run `bunx vercel@latest` from the user's project directory, where `.vercel/project.json` sets the active team and project. Never install the CLI globally or through npm, npx, pnpm, or yarn. Helpers live under the skill directory:

```bash
VERCEL_SKILL_DIR="${VERCEL_SKILL_DIR:-$HOME/.agents/skills/vercel-cli}"
```

Explicit user instructions override anything below.

## Access

`VERCEL_TOKEN` in the environment authenticates everything; it never appears in arguments, output, committed files, or generated `.env` files. `bash "$VERCEL_SKILL_DIR/scripts/vercel-ensure.sh"` checks Bun, the CLI, `jq`, `curl`, the token, local context, and authentication with one read-only request; run it only when the task needs live access. Missing dependencies are reported with an OS-appropriate install command, never installed automatically. `vercel login` and `vercel open` need the user's explicit request since they open a browser.

Target one-off commands with `--scope <team>` and explicit resource IDs. Link with `vercel link --project <p> --scope <team> --yes` only for repeated repository work, since it writes `.vercel/project.json`. Helpers resolve scope from an explicit argument, then `VERCEL_TEAM_ID`, `VERCEL_ORG_ID`, or `VERCEL_PROJECT_ID`, then `.vercel/project.json`.

## What is authorized

Read-only commands (`whoami`, `list`, `inspect`, `logs`, `env list`, `domains list`, `deploy --dry`) run without asking. Preview deploys run when the user asked to deploy. Production deploys, `promote`, `rollback`, cache deletion, and any deletion of projects, deployments, domains, aliases, env vars, Edge Config stores, webhooks, or bypass tokens need the user's explicit intent and one unambiguous target, inspected first; when several targets are plausible, ask rather than infer. An exact user request is authorization, so add no redundant confirmation.

Current `--help` beats any example here. For version-sensitive behavior beyond help, use Context7 with official Vercel docs.

## Native commands and helpers

The native command map, with deployments, logs, env, domains, aliases, Edge Config, webhooks, and `vercel api`, is in [references/commands.md](references/commands.md). Prefer native commands. Helpers add structured output, REST-only coverage, or non-interactive behavior:

| Helper | Use |
|---|---|
| `vercel-deploy.sh [--prod]` | Deploy with `{url, id, state, target}` JSON output for downstream inspection |
| `vercel-env.sh <action> ...` | Non-interactive, typed (`encrypted`, `plain`, `sensitive`), multi-target env CRUD; value `-` reads stdin, `@env:VAR` reads a variable |
| `vercel-bypass.sh <action> ...` | Deployment-protection bypass token lifecycle and protected fetches |
| `vercel-edge-config.sh` | Deterministic Edge Config item patches |
| `vercel-logs.sh` | Formatted build events and runtime log delegation |
| `vercel-webhooks.sh` | Webhook CRUD with project filtering |
| `vercel-api.sh <METHOD> <PATH> [json]` | Authenticated REST call for endpoints outside `vercel api list` |

Read [references/rest-api.md](references/rest-api.md) only for a REST gap, and [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy Vercel MCP workflow.

## Guardrails

- `vercel remove <project-name>` removes every deployment of the project; prefer a deployment ID and `--safe` when aliases must survive. `vercel projects remove` deletes the project itself.
- `promote` and `rollback` change production routing without deleting deployments. Inspect the target and the current production deployment first.
- Env changes apply to future deployments only; redeploy only when the user also wants the change live. Never echo secret values. `env pull` writes secrets to disk: confirm the destination, never overwrite implicitly, keep it out of Git.
- Bypass secrets grant access to protected deployments: send them as a header, never log them, revoke temporary tokens after use.
- Domain purchase, removal, and transfer change external ownership or routing. Inspect domain and team first.
- On HTTP 429 honor `Retry-After`; no unbounded retry loops.

Report the affected scope, resource, operation, and resulting state, with tokens, env values, bypass secrets, and secret-bearing URLs scrubbed.
