---
name: neon-cli
description: Operate Neon Postgres from the terminal with neonctl, psql, and bundled SQL helpers. Use when asked to query a Neon database, manage Neon projects or branches, or run or preview a migration on Neon.
---

# Neon CLI

Run `bunx neonctl@latest` from the user's project directory, where the nearest `.neon` context resolves. Never install `neonctl` globally or through npm, npx, pnpm, or yarn. Helpers live under the skill directory:

```bash
NEON_SKILL_DIR="${NEON_SKILL_DIR:-$HOME/.agents/skills/neon-cli}"
```

Explicit user instructions override anything below.

## Access

`NEON_API_KEY` in the environment authenticates everything. Keys, database passwords, connection strings, and generated `.env` values never appear in output, logs, or commits. `bash "$NEON_SKILL_DIR/scripts/neon-ensure.sh"` checks Bun, `neonctl`, `psql`, `jq`, the key, local context, and authentication with one read-only request; run it only when the task needs live access. Interactive `neonctl auth` opens a browser and needs the user's explicit request. Missing clients are reported with the OS-appropriate package, never installed automatically.

Some `neonctl` versions render environment-backed defaults in help output, so inspect help with the key removed: `env -u NEON_API_KEY bunx neonctl@latest <command> --help`. Current help beats any example here; for version-sensitive behavior beyond it, use Context7 with official Neon docs.

Target one-off commands with `--project-id "$PID"` and `--output json`. Link only for repeated repository work, and since `link` and `checkout` pull env vars into `.env` by default, pass `--no-env-pull` unless the user wants that:

```bash
bunx neonctl@latest link --project-id "$PID" --branch main --agent --no-env-pull
bunx neonctl@latest checkout feature/users --no-env-pull
```

`set-context` is deprecated. Helpers resolve the project from an explicit argument, then `NEON_PROJECT_ID`, then the nearest `.neon`.

## What is authorized

Reads (`projects list`, `branches list`, `databases list`, `cs`, `schema-diff`, `SELECT`, `EXPLAIN` on reads) run without asking. Branch creation and reviewed DDL or DML on a non-default branch run when the user asked for the change. Project deletion, branch deletion, reset, restore, `DROP`, and destructive DML need the user's explicit intent and an unambiguous target inspected first: for branch deletion, verify it is not the default branch; for reset or restore, keep the previous state under a backup name when rollback may matter. An exact user request is authorization, so add no redundant confirmation.

## Connections and helpers

Direct connection for DDL, migrations, `COPY`, `LISTEN/NOTIFY`, prepared statements, and session settings; pooled for ordinary reads and single-statement writes. The native command map is in [references/commands.md](references/commands.md); `psql`, transactions, JSON capture, and `EXPLAIN` details are in [references/sql-execution.md](references/sql-execution.md).

| Helper (args: branch, ..., [mode], [project]) | Default mode |
|---|---|
| `neon-sql.sh <branch> "<sql>"` | pooled |
| `neon-tx.sh <branch> -f <file.sql>` | direct |
| `neon-tables.sh <branch> [db]`, `neon-describe.sh <branch> <table> [schema] [db]` | pooled |
| `neon-explain.sh <branch> "<sql>" [--safe]` | direct |
| `neon-slow-queries.sh <branch> [limit] [db]` | pooled |

`EXPLAIN ANALYZE` executes the statement. `neon-explain.sh --safe` wraps mutating SQL in `BEGIN` and `ROLLBACK`; still inspect untrusted SQL for functions with external side effects before running it.

## Migration preview

For a non-trivial migration, apply it on a schema-only branch and diff before touching the target:

```bash
BRANCH="migration/$(date +%Y%m%d-%H%M%S)"
bunx neonctl@latest branches create --name "$BRANCH" --parent main --schema-only --project-id "$PID" --output json
bash "$NEON_SKILL_DIR/scripts/neon-tx.sh" "$BRANCH" -f migration.sql direct "$PID"
bunx neonctl@latest branches schema-diff main "$BRANCH" --project-id "$PID" --database neondb
```

Apply to the target branch only when the request includes that mutation; keep or delete the preview branch per the requested rollback window.

Read [references/management-api.md](references/management-api.md) when `neonctl api` must cover a CLI gap, and [references/mcp-parity.md](references/mcp-parity.md) only when translating a Neon MCP workflow.

Report project, branch, and operation outcome with secrets and connection strings scrubbed.
