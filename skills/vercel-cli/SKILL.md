---
name: vercel-cli
description: "Operate Vercel from a coding-agent terminal with the official Vercel CLI and bundled REST helpers. Use when the agent needs to inspect or manage Vercel teams, projects, deployments, build or runtime logs, environment variables, domains, aliases, Edge Config stores, webhooks, deployment-protection bypass tokens, log drains, promotions, rollbacks, or redeployments; when the user asks to deploy to Vercel, diagnose a Vercel deployment, check Vercel logs, manage Vercel environment variables, or invoke vercel-cli; or when translating a Vercel MCP workflow to shell commands. Do not use for authoring application code that merely runs on Vercel, application runtime integration through @vercel/sdk, continuous monitoring, or Vercel MCP server configuration."
---

# Vercel CLI

Operate Vercel through `bunx vercel@latest` and the bundled shell helpers. Keep the working directory in the user's project so Vercel can resolve its local `.vercel` context.

## Operating contract

1. Set the absolute directory containing this `SKILL.md` as `VERCEL_SKILL_DIR`. In Arthur's installation:

   ```bash
   VERCEL_SKILL_DIR=/home/arthur/.agents/skills/vercel-cli
   ```

2. Do not `cd` into the skill directory before running Vercel commands. The current project directory determines the active `.vercel/project.json` context.
3. Use `bunx vercel@latest`. Do not install Vercel CLI globally and do not use npm, npx, pnpm, or yarn.
4. Authenticate with `VERCEL_TOKEN` in the environment. Do not place the token in command arguments, output, committed files, or generated `.env` files.
5. Prefer native Vercel CLI commands. Use bundled helpers for structured deployment output, REST-only operations, or non-interactive bulk operations.
6. Keep browser use opt-in. Do not run `vercel login` or `vercel open` unless the user explicitly requested browser authentication or dashboard navigation.
7. Require explicit destructive intent and an unambiguous target before deleting projects, deployments, domains, aliases, environment variables, Edge Config stores, webhooks, or bypass tokens. Apply the same rule to production deploys, promotion, rollback, cache deletion, and domain purchase or transfer.
8. Inspect the current target before a production mutation. Do not infer team, project, environment, deployment, or domain scope from a destructive request with multiple plausible targets.
9. Do not install missing dependencies automatically. Detect the host OS, report the missing dependency, and provide the appropriate install command.

## Preflight

Run the bundled preflight only when the task needs live Vercel access:

```bash
bash "$VERCEL_SKILL_DIR/scripts/vercel-ensure.sh"
```

It checks Bun, the current Vercel CLI, `jq`, `curl`, `VERCEL_TOKEN`, local project context, and authentication. It performs one read-only authentication request.

For documentation-only work, skip the live preflight. Inspect current command syntax directly:

```bash
bunx vercel@latest <command> --help
```

For version-sensitive behavior not covered by current help, use Context7 with official Vercel documentation. Do not open a browser unless the user opted in.

## Authentication and targeting

Export a narrowly scoped token:

```bash
export VERCEL_TOKEN=vcp_xxxxxxxxxxxxxxxxxxxxxxxx
```

For one-off team operations, pass the scope explicitly:

```bash
bunx vercel@latest projects list --scope <team-slug-or-id>
```

For repeated repository work, link the project non-interactively:

```bash
bunx vercel@latest link --project <project-name-or-id> --scope <team-slug-or-id> --yes
```

`link` writes `.vercel/project.json`. Do not link merely to execute one account-level command when `--scope`, `--project`, or an explicit resource identifier is sufficient.

Bundled helpers resolve scope in this order:

1. Explicit script argument
2. `VERCEL_TEAM_ID`, `VERCEL_ORG_ID`, or `VERCEL_PROJECT_ID`
3. `.vercel/project.json` in the current working directory

## Execution workflow

1. Classify the request as read-only, mutating, production-affecting, or destructive.
2. Resolve the team, project, environment, and resource identifier with read-only commands.
3. Inspect current state before mutation. For deployment changes, inspect both the target deployment and the current production deployment.
4. Use a native command from [references/commands.md](references/commands.md). Use a helper only when it adds required REST coverage, structured output, or non-interactive behavior.
5. Execute the narrowest command that satisfies the request. Avoid project-wide commands when a deployment ID or resource ID exists.
6. Report the affected scope, resource, operation, and resulting state. Scrub tokens, environment values, bypass secrets, URLs containing secret query parameters, and sensitive response fields.

## Quick map

| Intent | Command |
|---|---|
| Verify authentication | `bunx vercel@latest whoami` |
| List teams | `bunx vercel@latest teams list` |
| List projects | `bunx vercel@latest projects list` |
| Inspect a project | `bunx vercel@latest projects inspect <name-or-id>` |
| List deployments | `bunx vercel@latest list [project]` |
| Inspect a deployment as JSON | `bunx vercel@latest inspect <url-or-id> --format=json` |
| Read build logs | `bunx vercel@latest inspect <url-or-id> --logs` |
| Read recent runtime logs | `bunx vercel@latest logs <url-or-id> --json` |
| Follow runtime logs | `bunx vercel@latest logs <url-or-id> --follow` |
| Deploy preview with structured output | `bash "$VERCEL_SKILL_DIR/scripts/vercel-deploy.sh"` |
| Deploy production with structured output | `bash "$VERCEL_SKILL_DIR/scripts/vercel-deploy.sh" --prod` |
| Promote a deployment | `bunx vercel@latest promote <url-or-id>` |
| Roll back production | `bunx vercel@latest rollback <url-or-id>` |
| Redeploy a prior deployment | `bunx vercel@latest redeploy <url-or-id>` |
| List environment variables | `bunx vercel@latest env list [environment]` |
| Pull variables to a chosen file | `bunx vercel@latest env pull <filename> --environment=<environment>` |
| Perform non-interactive env CRUD | `bash "$VERCEL_SKILL_DIR/scripts/vercel-env.sh" <action> ...` |
| List or inspect domains | `bunx vercel@latest domains list` or `domains inspect <domain>` |
| Check domain availability and price | `bunx vercel@latest domains check <domain>` and `domains price <domain>` |
| Manage aliases | `bunx vercel@latest alias <list|set|remove> ...` |
| Manage Edge Config | `bunx vercel@latest edge-config <command> ...` |
| Manage webhooks | `bunx vercel@latest webhooks <command> ...` |
| Create or revoke a bypass token | `bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" <action> ...` |
| Call a REST endpoint | `bunx vercel@latest api <endpoint> [options]` |
| Call a REST gap with raw JSON | `bash "$VERCEL_SKILL_DIR/scripts/vercel-api.sh" <METHOD> <PATH> [json-body]` |

## Deployment workflow

Use `--dry` when the user wants to inspect deployment inputs without creating a deployment:

```bash
bunx vercel@latest deploy --dry --format=json
```

For an authorized deploy, preserve structured output for downstream inspection:

```bash
DEPLOYMENT=$(bash "$VERCEL_SKILL_DIR/scripts/vercel-deploy.sh")
DEPLOYMENT_URL=$(printf '%s' "$DEPLOYMENT" | jq -r '.url')
bunx vercel@latest inspect "$DEPLOYMENT_URL" --format=json
```

Use `--prod` only when production deployment is explicit. Before `promote` or `rollback`, inspect the target and the current production list. These operations change production routing even though they do not delete the underlying deployments.

## Environment-variable workflow

List keys and metadata without returning values whenever possible. Never echo or summarize secret values.

Use native `env` commands for ordinary linked-project work. Use `vercel-env.sh` when the request needs non-interactive values, multiple targets, or explicit `encrypted`, `plain`, or `sensitive` types:

```bash
bash "$VERCEL_SKILL_DIR/scripts/vercel-env.sh" add \
  <project> <KEY> @env:<SOURCE_VARIABLE> production preview sensitive
```

Use `-` as the value argument to read from stdin when the value is already held in a shell variable. Avoid literal secrets in process arguments.

Treat `env pull` as a secret-bearing file write. Confirm the destination, avoid overwriting an existing file implicitly, and keep the result out of Git.

## REST and helper boundaries

Prefer `vercel api` for endpoints represented in the CLI's current OpenAPI catalog. Use `vercel api list` to discover them and `--generate=curl` to inspect a request without executing it.

Use the bundled REST scripts when they provide a safer or more deterministic interface:

- `vercel-api.sh`: generic authenticated REST call with JSON output
- `vercel-bypass.sh`: deployment-protection bypass token lifecycle and protected fetches
- `vercel-deploy.sh`: deployment with structured `{url, id, state, target}` output
- `vercel-edge-config.sh`: deterministic Edge Config item patch operations
- `vercel-env.sh`: non-interactive, typed, multi-target environment-variable CRUD
- `vercel-logs.sh`: formatted build events and runtime log delegation
- `vercel-webhooks.sh`: webhook CRUD with optional project filtering

Read [references/rest-api.md](references/rest-api.md) only for a REST gap or endpoint contract. Read [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy Vercel MCP workflow.

## Guardrails

- `vercel remove <project-name>` removes every deployment for that project. Prefer a deployment ID. Use `--safe` when active aliases must be preserved.
- `vercel projects remove <name>` deletes the project. Do not confuse it with deployment removal.
- Environment-variable changes affect future deployments. Redeploy only when the user also intends to apply the change.
- Bypass secrets grant access to protected deployments. Prefer the request header over query parameters, never log the secret, and revoke temporary tokens after use.
- Domain purchase, removal, and transfer affect external ownership or routing. Inspect the domain and target team before mutation.
- For HTTP 429 responses, honor `Retry-After` when present. Do not create an unbounded retry loop.
- For flags not documented in this skill, inspect current `--help` rather than guessing.
