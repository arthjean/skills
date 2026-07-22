---
name: neon-cli
description: "Operate Neon Postgres from a coding-agent terminal with the official neonctl CLI and psql, without relying on the Neon MCP server. Covers projects, local project linking, branches, databases, roles, connection strings, SQL execution, transactions, schema diffs, migrations, EXPLAIN, slow-query inspection, and Management API gaps. Use when the agent needs to inspect, query, or modify a Neon database; create, restore, reset, diff, or delete branches; run migrations; manage Neon projects or roles; or when the user says neon-cli, neonctl, query my Neon DB, create a Neon branch, or list my Neon projects. Do not use for application runtime integration with Neon, ORM schema design, continuous monitoring, or Neon MCP server configuration."
---

# Neon CLI

Operate Neon through `bunx neonctl@latest`, `psql`, and the bundled shell helpers. Keep the working directory in the user's project so `neonctl` can resolve the nearest local `.neon` context.

## Operating contract

1. Use the absolute directory containing this `SKILL.md` as `NEON_SKILL_DIR`. In Arthur's installation:

   ```bash
   NEON_SKILL_DIR=/home/arthur/.agents/skills/neon-cli
   ```

2. Do not `cd` into the skill directory before running Neon commands. The current project directory determines which `.neon` context is active.
3. Use `bunx neonctl@latest`. Do not install `neonctl` globally and do not use npm, npx, pnpm, or yarn.
4. Prefer native `neonctl` commands. Use bundled scripts only for SQL workflows that need `psql`.
5. Preserve secrets. Never print, log, commit, or return `NEON_API_KEY`, database passwords, connection strings, or generated `.env` values.
6. Keep browser use opt-in. Do not run interactive `neonctl auth` unless the user explicitly requested browser authentication.
7. Require an explicit destructive intent and an unambiguous target before project deletion, branch deletion, reset, restore, `DROP`, or destructive DML. Do not infer these actions from a general request to manage or fix a database.

## Preflight

Run the bundled preflight only when the task actually needs Neon access:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-ensure.sh"
```

It checks `bun`, current `neonctl`, `psql`, `jq`, `NEON_API_KEY`, local context, and authentication. It performs one read-only authentication request.

If inspecting CLI help, remove the API key from that subprocess because some `neonctl` versions render environment-backed defaults in help output:

```bash
env -u NEON_API_KEY bunx neonctl@latest branches create --help
```

Do not run install commands automatically. Detect the host OS first, then tell the user which missing client package is required.

## Authentication and project targeting

Use a scoped API key through the environment:

```bash
export NEON_API_KEY=neon_api_xxxxxxxxxxxx
```

For a one-off operation, pass the project explicitly:

```bash
PID=polished-wind-123456
bunx neonctl@latest branches list --project-id "$PID" --output json
```

For repeated repository work, link the project non-interactively. `link` and `checkout` pull environment variables into `.env` by default, so disable that unless the user explicitly wants it:

```bash
bunx neonctl@latest link \
  --project-id "$PID" \
  --branch main \
  --agent \
  --no-env-pull

bunx neonctl@latest checkout feature/users --no-env-pull
```

`set-context` is deprecated. Use `link` and `checkout`. Do not create a `.neon` file merely to execute one command when `--project-id` is sufficient.

Bundled scripts resolve the project in this order:

1. Explicit script argument
2. `NEON_PROJECT_ID`
3. The nearest `.neon` file resolved by `neonctl`

## Execution workflow

1. Identify the project, branch, database, role, and whether the request is read-only or mutating.
2. Resolve missing targets with read-only commands such as `projects list`, `branches list`, or `databases list`.
3. Use a native command from [references/commands.md](references/commands.md), or a bundled SQL helper from [references/sql-execution.md](references/sql-execution.md).
4. Before a destructive action, inspect the exact target. For branch deletion, verify it is not the default branch. For reset or restore, preserve the previous state under a backup name when rollback may matter.
5. Execute the narrowest command that satisfies the request.
6. Report the affected project, branch, and operation outcome. Scrub secrets and connection strings from all output.

## Quick map

| Intent | Command |
|---|---|
| List projects | `bunx neonctl@latest projects list --output json` |
| List branches | `bunx neonctl@latest branches list --project-id "$PID" --output json` |
| Get a direct connection string | `bunx neonctl@latest cs main --project-id "$PID" --no-color` |
| Open psql | `bunx neonctl@latest psql main --project-id "$PID"` |
| Run one SQL statement | `bash "$NEON_SKILL_DIR/scripts/neon-sql.sh" main "SELECT count(*) FROM users" pooled "$PID"` |
| Run a transaction file | `bash "$NEON_SKILL_DIR/scripts/neon-tx.sh" main -f migration.sql direct "$PID"` |
| List tables | `bash "$NEON_SKILL_DIR/scripts/neon-tables.sh" main neondb "$PID"` |
| Describe a table | `bash "$NEON_SKILL_DIR/scripts/neon-describe.sh" main users public neondb "$PID"` |
| Explain a read query | `bash "$NEON_SKILL_DIR/scripts/neon-explain.sh" main "SELECT * FROM users" "$PID"` |
| Inspect slow queries | `bash "$NEON_SKILL_DIR/scripts/neon-slow-queries.sh" main 20 neondb "$PID"` |
| Create a branch | `bunx neonctl@latest branches create --name feature/users --parent main --project-id "$PID" --output json` |
| Diff schemas | `bunx neonctl@latest branches schema-diff main feature/users --project-id "$PID" --database neondb` |
| Call an API route | `bunx neonctl@latest api "/projects/$PID/operations" --output json` |

## Connection choice

Use a direct connection for DDL, migrations, `COPY`, `LISTEN/NOTIFY`, prepared statements, and session-scoped settings. Use a pooled connection for ordinary application-style reads and single-statement writes.

The SQL helpers choose these defaults:

| Helper | Default |
|---|---|
| `neon-sql.sh` | pooled |
| `neon-tx.sh` | direct |
| `neon-tables.sh` | pooled |
| `neon-describe.sh` | pooled |
| `neon-explain.sh` | direct |
| `neon-slow-queries.sh` | pooled |

`EXPLAIN ANALYZE` executes the statement. Use `neon-explain.sh --safe` for mutating SQL so the helper wraps it in `BEGIN` and `ROLLBACK`, but still inspect untrusted SQL for functions or external side effects before execution.

## Migration workflow

Use a temporary branch to preview non-trivial migrations:

```bash
PID=polished-wind-123456
BRANCH="migration/$(date +%Y%m%d-%H%M%S)"

bunx neonctl@latest branches create \
  --name "$BRANCH" \
  --parent main \
  --schema-only \
  --project-id "$PID" \
  --output json

bash "$NEON_SKILL_DIR/scripts/neon-tx.sh" \
  "$BRANCH" -f migration.sql direct "$PID"

bunx neonctl@latest branches schema-diff main "$BRANCH" \
  --project-id "$PID" \
  --database neondb
```

Apply the reviewed migration to the target branch only when the user's request includes that mutation. Keep or delete the preview branch according to the requested rollback window.

## References

- Read [references/commands.md](references/commands.md) for the current command families and safe examples.
- Read [references/sql-execution.md](references/sql-execution.md) for `psql`, transactions, JSON capture, `EXPLAIN`, and connection-mode details.
- Read [references/management-api.md](references/management-api.md) when `neonctl api` is needed for a CLI parity gap.
- Read [references/mcp-parity.md](references/mcp-parity.md) only when translating a Neon MCP workflow to CLI commands.

For flags not covered here, inspect current help with `env -u NEON_API_KEY bunx neonctl@latest <command> --help`. For version-sensitive Neon behavior, use Context7 or official Neon documentation rather than relying on this reference indefinitely.
