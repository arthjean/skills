# neonctl command reference

Use `bunx neonctl@latest` from the user's project directory. Keep `NEON_API_KEY` in the environment and quote every target variable.

Before relying on an unfamiliar flag, inspect current help without forwarding the API key:

```bash
env -u NEON_API_KEY bunx neonctl@latest branches create --help
```

## Global behavior

| Flag | Purpose |
|---|---|
| `--output json` | Machine-readable output |
| `--project-id <id>` | Explicit project target |
| `--context-file <path>` | Override the nearest `.neon` context |
| `--org-id <id>` | Explicit organization target where supported |
| `--no-color` | Stable captured output |
| `--no-analytics` | Disable CLI analytics for the command |

Do not pass `--api-key` on the command line. Use `NEON_API_KEY` so the key is not stored in shell history or exposed in process arguments.

## Local context

`link` replaces the deprecated `set-context`. Use agent mode and disable the default environment pull unless the user asked to write `.env`:

```bash
bunx neonctl@latest link \
  --project-id "$PID" \
  --branch main \
  --agent \
  --no-env-pull

bunx neonctl@latest checkout feature/users --no-env-pull

bunx neonctl@latest link --clear --agent
```

For a one-off command, prefer `--project-id "$PID"` and avoid changing repository context.

## Projects

```bash
bunx neonctl@latest projects list --output json
bunx neonctl@latest projects get "$PID" --output json

bunx neonctl@latest projects create \
  --name my-app-db \
  --region-id "$REGION" \
  --database neondb \
  --role neondb_owner \
  --org-id "$ORG_ID" \
  --output json

bunx neonctl@latest projects update "$PID" \
  --name renamed-project \
  --cu 1-4 \
  --output json

bunx neonctl@latest projects delete "$PID"
bunx neonctl@latest projects recover "$PID"
```

Project deletion is destructive. Resolve the project with `projects get` immediately before deletion and require the user to have named that deletion explicitly.

Region identifiers and project options change over time. Read `projects create --help` before creating a project when the requested region or compute shape is not already known.

## Branches

```bash
bunx neonctl@latest branches list \
  --project-id "$PID" \
  --output json

bunx neonctl@latest branches get feature/users \
  --project-id "$PID" \
  --output json

bunx neonctl@latest branches create \
  --project-id "$PID" \
  --name feature/users \
  --parent main \
  --type read_write \
  --suspend-timeout 300 \
  --output json

bunx neonctl@latest branches create \
  --project-id "$PID" \
  --name migration-preview \
  --parent main \
  --schema-only \
  --output json

bunx neonctl@latest branches create \
  --project-id "$PID" \
  --name restore-point \
  --parent "main@2026-07-15T00:00:00Z" \
  --output json
```

Schema comparison:

```bash
bunx neonctl@latest branches schema-diff main feature/users \
  --project-id "$PID" \
  --database neondb

bunx neonctl@latest branches schema-diff feature/users ^parent \
  --project-id "$PID"

bunx neonctl@latest branches schema-diff feature/users "^self@0/1A2B3C" \
  --project-id "$PID"
```

Reset, restore, and deletion are destructive:

```bash
bunx neonctl@latest branches reset feature/users \
  --parent \
  --preserve-under-name feature/users-backup \
  --project-id "$PID"

bunx neonctl@latest branches restore main "source@2026-07-15T00:00:00Z" \
  --preserve-under-name main-before-restore \
  --project-id "$PID"

bunx neonctl@latest branches delete feature/users \
  --project-id "$PID"
```

Before deletion, inspect `branches list --output json` and identify the default branch from the returned payload. Do not assume its name is `main`.

Other branch operations:

```bash
bunx neonctl@latest branches rename old-name new-name --project-id "$PID"
bunx neonctl@latest branches set-default new-name --project-id "$PID"
bunx neonctl@latest branches set-expiration preview \
  --expires-at 2026-07-16T00:00:00Z \
  --project-id "$PID"
bunx neonctl@latest branches add-compute feature/users --project-id "$PID"
```

## Databases

```bash
bunx neonctl@latest databases list \
  --project-id "$PID" \
  --branch main \
  --output json

bunx neonctl@latest databases create \
  --project-id "$PID" \
  --branch main \
  --name analytics \
  --owner-name neondb_owner \
  --output json

bunx neonctl@latest databases delete analytics \
  --project-id "$PID" \
  --branch main
```

## Roles

```bash
bunx neonctl@latest roles list \
  --project-id "$PID" \
  --branch main \
  --output json

bunx neonctl@latest roles create \
  --project-id "$PID" \
  --branch main \
  --name readonly_api \
  --output json

bunx neonctl@latest roles create \
  --project-id "$PID" \
  --branch main \
  --name app_role \
  --no-login \
  --output json

bunx neonctl@latest roles delete readonly_api \
  --project-id "$PID" \
  --branch main
```

Role creation output may contain a password. Capture it only when the user needs it, scrub it from the response, and never persist it without an explicit destination.

## Connection strings and psql

```bash
bunx neonctl@latest cs main \
  --project-id "$PID" \
  --no-color

bunx neonctl@latest cs feature/users \
  --project-id "$PID" \
  --role-name neondb_owner \
  --database-name neondb \
  --pooled \
  --ssl require \
  --no-color

bunx neonctl@latest psql main \
  --project-id "$PID"

bunx neonctl@latest psql main \
  --project-id "$PID" \
  -- -v ON_ERROR_STOP=1 -c "SELECT version();"
```

A connection string contains credentials. Capture it into a shell variable when needed and never echo it.

## Operations, allowlists, organizations, and user

```bash
bunx neonctl@latest operations list \
  --project-id "$PID" \
  --output json

bunx neonctl@latest ip-allow list \
  --project-id "$PID" \
  --output json

bunx neonctl@latest ip-allow add 203.0.113.10 \
  --project-id "$PID"

bunx neonctl@latest ip-allow remove 203.0.113.10 \
  --project-id "$PID"

bunx neonctl@latest orgs list --output json
bunx neonctl@latest me --output json
```

`ip-allow reset` replaces the allowlist and is destructive. Inspect the current list first.

## Management API passthrough

Use the authenticated native passthrough before writing raw `curl`:

```bash
bunx neonctl@latest api --list
bunx neonctl@latest api "/projects/$PID" --output json
bunx neonctl@latest api "/projects/$PID/endpoints" --output json
```

See [management-api.md](management-api.md) for request bodies and route discovery.

## Authentication

For Codex automation, use `NEON_API_KEY`. `neonctl auth` is interactive and may open a browser, so do not invoke it unless the user explicitly requests that flow.
