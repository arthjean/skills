---
name: posthog-cli
description: "Operate PostHog from a coding-agent terminal with the official @posthog/cli agent API and bundled REST helpers. Use when the agent needs to inspect, query, create, update, or delete PostHog insights, dashboards, feature flags, experiments, cohorts, persons, events, surveys, recordings, error-tracking issues, warehouse resources, CDP functions, LLM analytics, organizations, or projects; run HogQL or typed analytics queries; upload source maps or debug symbols; translate a PostHog MCP workflow; or when the user says posthog-cli, PostHog CLI, PostHog API, HogQL, query my PostHog data, or manage my PostHog flags. Do not use for application runtime instrumentation through PostHog SDKs, continuous polling, PostHog MCP server configuration, or dashboard-only billing and instance administration."
---

# PostHog CLI

Operate PostHog through the official `@posthog/cli` agent API. Keep the bundled Bash helpers for REST gaps, deterministic pipelines, and HogQL file or tabular workflows.

## Operating contract

1. Set the absolute directory containing this `SKILL.md` as `POSTHOG_SKILL_DIR`. In Arthur's installation:

   ```bash
   POSTHOG_SKILL_DIR=/home/arthur/.agents/skills/posthog-cli
   ```

2. Keep the working directory in the user's project. The helpers discover the nearest `.env.local` or `.env` from `$PWD`; changing into the skill directory selects the wrong context.
3. Use `bunx --bun @posthog/cli@latest`. Do not install the CLI globally and do not use npm, npx, pnpm, or yarn.
4. Prefer `posthog-cli api` over raw REST. It exposes the current agent-oriented tool catalog, schemas, dry-run validation, and destructive-operation checks.
5. Keep credentials in the environment. Never print, log, commit, or return a personal API key, locally stored login token, authorization header, or secret-bearing response field.
6. Keep browser use opt-in. Do not run `login`, open dashboard URLs, or invoke browser authentication unless the user explicitly requested it.
7. Require explicit intent and an unambiguous organization, project, environment, and resource before any create, update, delete, rollout, experiment transition, cohort membership change, bulk operation, or other mutation. An exact user request is authorization; do not add a redundant confirmation.
8. Inspect current state before destructive or wide-impact actions. Add `--confirm` only after checking the exact target IDs. Never infer it from a general request to inspect or manage PostHog.
9. Do not run `api skill install` or `api agents-md install` unless the user explicitly asked to modify agent or repository configuration.
10. Do not install missing dependencies automatically. Detect the host environment and report the missing executable with an appropriate install command.

## Preflight

Run the bundled preflight only when the task needs live PostHog access:

```bash
bash "$POSTHOG_SKILL_DIR/scripts/posthog-ensure.sh"
```

It checks Bash, Bun, the current PostHog CLI, `curl`, `jq`, credential shape, host, project context, and authentication through a read-only request.

For documentation or schema discovery, skip live authentication. Inspect current local CLI guidance with credentials removed from the subprocess:

```bash
env -u POSTHOG_PERSONAL_API_KEY \
    -u POSTHOG_API_KEY \
    -u POSTHOG_CLI_API_KEY \
    bunx --bun @posthog/cli@latest api --agent-help
```

For version-sensitive behavior not covered by current help or tool schemas, use Context7 with official PostHog documentation. Do not guess a flag, schema, or raw endpoint.

## Authentication and project context

For headless agent use, prefer the official CLI variables:

```bash
export POSTHOG_CLI_API_KEY=phx_xxxxxxxxxxxxxxxxxxxx
export POSTHOG_CLI_PROJECT_ID=12345
export POSTHOG_CLI_HOST=https://us.posthog.com
```

Use `https://eu.posthog.com` for EU Cloud or the exact origin of a self-hosted instance. The bundled helpers also accept `POSTHOG_PERSONAL_API_KEY`, `POSTHOG_PROJECT_ID`, and `POSTHOG_HOST`, and map the CLI variable names to those internal names.

Project API keys beginning with `phc_` are ingestion credentials and cannot operate the private management API. Use a scoped personal key for the resources required by the task. Before a mutation, verify the key source, active host, and target project reported by the preflight or a read-only tool call.

Do not run interactive `login` merely because a key is missing. Report the missing credential or use browser authentication only when the user explicitly chose that flow.

## Agent API workflow

1. Find an unknown tool with a narrow search. Fall back to `tools` only when no useful search term exists.
2. Run `info` once for every tool whose schema is not already in context.
3. If `info` returns a `hint`, inspect that field with `schema`. Continue drilling into nested hints before building the input.
4. For analytics over events, persons, sessions, or properties, inspect `read-data-schema` before querying. Do not assume canonical-looking event or property names exist in the active project.
5. Prefer typed `query-*` tools when they express the requested analysis. Use `execute-sql` only for joins, window functions, warehouse queries, or entity searches that typed queries cannot represent.
6. Before a mutation, run `call --dry-run` with the exact payload, inspect the target, then execute once. Add `--confirm` only when the CLI identifies the operation as destructive and the user's request authorizes it.
7. Use `--json` when another command will consume the result. Report the affected scope and resulting state, with credentials and PII scrubbed.

```bash
bunx --bun @posthog/cli@latest api search feature-flag
bunx --bun @posthog/cli@latest api info feature-flag-get-all
bunx --bun @posthog/cli@latest api call --json feature-flag-get-all '{}'

bunx --bun @posthog/cli@latest api info update-feature-flag
bunx --bun @posthog/cli@latest api call --dry-run update-feature-flag '{"id":123,"active":false}'
```

Never reuse example input blindly. Tool names and fields can change; current `info` and `schema` output are authoritative for the installed CLI.

## Helper boundary

Use a bundled helper only when it adds something the official agent API does not provide cleanly:

| Need | Command |
|---|---|
| Verify live context | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-ensure.sh"` |
| Call a verified REST path | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-api.sh" <METHOD> <PATH> [json-body]` |
| Run HogQL from a file | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" hogql-file query.sql` |
| Produce tabular HogQL output | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" table '<sql>'` |
| Run and poll an async HogQL query | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" async '<sql>'` |
| List projects through direct REST | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh" ls` |
| Use a resource convenience wrapper | `bash "$POSTHOG_SKILL_DIR/scripts/posthog-<resource>.sh" <subcommand> ...` |
| Upload source maps or symbols | `bunx --bun @posthog/cli@latest <sourcemap|dsym|hermes|proguard|symbol-sets> ...` |

Keep `$PWD` in the user project when invoking helpers by absolute path. Helpers resolve context in this order:

1. Explicit project ID argument
2. Existing environment variable
3. Nearest `.env.local`, then `.env`, walking upward from `$PWD`

Treat raw REST references as a fallback snapshot, not a permanent contract. Verify a path and body against current PostHog documentation before adding or changing a helper.

## Guardrails

- Person deletion is irreversible. Events remain, but identity linkage is lost. Inspect every person ID and the total count before deletion.
- Feature flag changes can affect production immediately. Inspect the current definition, rollout, dependencies, and target project before mutation.
- Shipping or ending an experiment changes its lifecycle and may not be reversible. Inspect current status and winning variant before execution.
- Static cohort membership helpers do not apply to dynamic cohorts. Inspect cohort type first.
- For bulk operations, materialize the target IDs, review count and sample, pace requests, and stop on the first unexpected response. Never pipe an unreviewed list directly into mutations.
- Honor `Retry-After` on HTTP 429. Do not create unbounded retries or continuous polling loops.
- Treat event properties, person data, recordings, errors, logs, and LLM traces as potentially sensitive. Retrieve only fields needed for the task and scrub PII from the final response.
- Never pass a personal API key in a command argument or URL. Keep it in the environment so it is absent from shell history and process listings.

## References

- Read [references/commands.md](references/commands.md) for current native CLI families and the full helper subcommand inventory.
- Read [references/hogql-cookbook.md](references/hogql-cookbook.md) only when a typed query cannot express the analysis and raw HogQL is justified.
- Read [references/rest-api.md](references/rest-api.md) only for a verified REST gap.
- Read [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy PostHog MCP or helper workflow.

For unknown native commands, inspect `bunx --bun @posthog/cli@latest <command> --help`. For unknown agent tools, use `api search`, `api info`, and `api schema` rather than relying on stale examples.
