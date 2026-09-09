---
name: posthog-cli
description: Operate PostHog from the terminal with the @posthog/cli agent API, HogQL, and bundled REST helpers. Use when asked to query PostHog data or manage flags, insights, dashboards, experiments, cohorts, surveys, or error tracking.
---

# PostHog CLI

Run `bunx --bun @posthog/cli@latest` from the user's project directory, where helpers discover the nearest `.env.local` or `.env`. Never install the CLI globally or through npm, npx, pnpm, or yarn. Helpers live under the skill directory:

```bash
POSTHOG_SKILL_DIR="${POSTHOG_SKILL_DIR:-$HOME/.agents/skills/posthog-cli}"
```

Explicit user instructions override anything below.

## Access

Headless use takes `POSTHOG_CLI_API_KEY` (a scoped personal key, `phx_`), `POSTHOG_CLI_PROJECT_ID`, and `POSTHOG_CLI_HOST` (`https://us.posthog.com`, `https://eu.posthog.com`, or the self-hosted origin). Helpers also accept `POSTHOG_PERSONAL_API_KEY`, `POSTHOG_PROJECT_ID`, and `POSTHOG_HOST`, resolving project from an explicit argument, then the environment, then the nearest env file. A `phc_` project key is an ingestion credential and cannot operate the management API.

`bash "$POSTHOG_SKILL_DIR/scripts/posthog-ensure.sh"` checks Bun, the CLI, `curl`, `jq`, key shape, host, project, and authentication with one read-only request; run it only when the task needs live access. Keys never appear in arguments, URLs, output, or commits. `login`, dashboard URLs, `api skill install`, and `api agents-md install` need the user's explicit request. Missing dependencies are reported, never installed automatically.

For schema discovery without credentials in the subprocess:

```bash
env -u POSTHOG_PERSONAL_API_KEY -u POSTHOG_API_KEY -u POSTHOG_CLI_API_KEY \
    bunx --bun @posthog/cli@latest api --agent-help
```

## Agent API

`api search <term>`, then `api info <tool>` for every tool whose schema is not in context, then `api schema` on any `hint` it returns, nested hints included. Current `info` and `schema` output is authoritative; tool names and fields change, so never reuse an example blindly.

```bash
bunx --bun @posthog/cli@latest api search feature-flag
bunx --bun @posthog/cli@latest api info feature-flag-get-all
bunx --bun @posthog/cli@latest api call --json feature-flag-get-all '{}'
bunx --bun @posthog/cli@latest api call --dry-run update-feature-flag '{"id":123,"active":false}'
```

For analytics, read `read-data-schema` first: canonical-looking event and property names may not exist in the active project. Prefer typed `query-*` tools; `execute-sql` (HogQL) is for joins, window functions, warehouse queries, or entity searches typed queries cannot express. `--json` when another command consumes the result.

## What is authorized

Reads and queries run without asking. Any create, update, delete, rollout change, experiment transition, cohort membership change, or bulk operation needs the user's explicit intent and an unambiguous organization, project, and resource: run `call --dry-run` with the exact payload, inspect the target, execute once, and add `--confirm` only when the CLI flags the operation destructive. An exact user request is authorization, so add no redundant confirmation.

## Helpers and references

| Helper | Use |
|---|---|
| `posthog-query.sh hogql-file <file>` / `table '<sql>'` / `async '<sql>'` | HogQL from a file, tabular output, or async run-and-poll |
| `posthog-api.sh <METHOD> <PATH> [json]` | One verified REST path outside the agent API |
| `posthog-<resource>.sh <subcommand>` | Convenience wrappers per resource (flags, cohorts, persons, insights, ...), inventoried in [references/commands.md](references/commands.md) |
| `bunx --bun @posthog/cli@latest <sourcemap\|dsym\|hermes\|proguard\|symbol-sets>` | Source map and symbol uploads, native |

Read [references/hogql-cookbook.md](references/hogql-cookbook.md) only when raw HogQL is justified, [references/rest-api.md](references/rest-api.md) only for a verified REST gap, and [references/mcp-parity.md](references/mcp-parity.md) only when translating a legacy PostHog MCP workflow. For version-sensitive behavior beyond current help, use Context7 with official PostHog docs.

## Guardrails

- Person deletion is irreversible: events remain, identity linkage is lost. Inspect every ID and the count first.
- Flag changes hit production immediately. Inspect definition, rollout, dependencies, and project first.
- Shipping or ending an experiment may not be reversible. Inspect status and winning variant first.
- Static cohort membership helpers do not apply to dynamic cohorts; check cohort type.
- Bulk operations: materialize the IDs, review count and sample, pace requests, stop on the first unexpected response. Never pipe an unreviewed list into mutations.
- Honor `Retry-After` on 429; no unbounded retries or polling loops.
- Event properties, person data, recordings, errors, logs, and LLM traces are potentially sensitive: fetch only needed fields, scrub PII from the report.

Report the affected scope and resulting state with credentials and PII scrubbed.
