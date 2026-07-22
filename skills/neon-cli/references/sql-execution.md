# SQL execution with neonctl and psql

Use the bundled helpers for repeatable SQL operations. They obtain a connection string through `bunx neonctl@latest cs` and never print it.

## Native one-off execution

Current `neonctl` exposes a `psql` command:

```bash
bunx neonctl@latest psql main \
  --project-id "$PID" \
  -- -v ON_ERROR_STOP=1 -c "SELECT count(*) FROM users;"
```

Use the helpers when output must be normalized, a transaction file must be applied, or a recurring inspection query is needed.

## Bundled helpers

Set the skill directory once:

```bash
NEON_SKILL_DIR=/home/arthur/.agents/skills/neon-cli
```

Single statement:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-sql.sh" \
  main "SELECT count(*) FROM users" pooled "$PID"
```

Transaction from a file:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-tx.sh" \
  main -f migration.sql direct "$PID"
```

Transaction from standard input:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-tx.sh" main direct "$PID" <<'SQL'
ALTER TABLE users ADD COLUMN verified_at TIMESTAMPTZ;
UPDATE users
SET verified_at = created_at
WHERE email_verified = true;
SQL
```

The quoted `SQL` delimiter prevents shell interpolation.

Schema inspection:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-tables.sh" \
  main neondb "$PID"

bash "$NEON_SKILL_DIR/scripts/neon-describe.sh" \
  main users public neondb "$PID"
```

Query plan:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-explain.sh" \
  main "SELECT * FROM users WHERE email = 'x@example.com'" "$PID"
```

`EXPLAIN ANALYZE` executes the statement. For mutating SQL:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-explain.sh" \
  main "DELETE FROM users WHERE inactive = true" --safe "$PID"
```

Slow statements:

```bash
bash "$NEON_SKILL_DIR/scripts/neon-slow-queries.sh" \
  main 20 neondb "$PID"
```

This query requires `pg_stat_statements`.

## Direct psql patterns

Capture the connection string without displaying it:

```bash
CONN_DIRECT="$(bunx neonctl@latest cs main \
  --project-id "$PID" \
  --database-name neondb \
  --no-color)"

psql "$CONN_DIRECT" -v ON_ERROR_STOP=1 -c "SELECT count(*) FROM users;"
```

Run a file in one transaction:

```bash
psql "$CONN_DIRECT" \
  -v ON_ERROR_STOP=1 \
  -1 \
  -f migration.sql
```

Capture JSON:

```bash
ROWS="$(
  psql "$CONN_DIRECT" -At -c \
    "SELECT COALESCE(json_agg(t), '[]'::json)
     FROM (SELECT id, email FROM users LIMIT 10) t;"
)"

printf '%s\n' "$ROWS" | jq '.[] | .email'
```

Bulk import and export use client-side `\copy` because the remote compute cannot read local paths:

```bash
psql "$CONN_DIRECT" \
  -c "\copy users TO 'users.csv' WITH CSV HEADER"

psql "$CONN_DIRECT" \
  -c "\copy users FROM 'users.csv' WITH CSV HEADER"
```

## Pooled versus direct

| Operation | Connection |
|---|---|
| Ordinary `SELECT` | pooled |
| Single-statement `INSERT`, `UPDATE`, or `DELETE` | pooled |
| Multi-statement application transaction | pooled when no session state is required |
| `CREATE`, `ALTER`, or `DROP` | direct |
| Migration | direct |
| `COPY` or `\copy` | direct |
| `LISTEN/NOTIFY` | direct |
| Prepared statements | direct |
| Session-scoped `SET` | direct |

## Safety

- Pass untrusted values as `psql` variables or query parameters. Do not concatenate them into SQL.
- Treat table, schema, role, and database identifiers as untrusted input.
- Use `-v ON_ERROR_STOP=1` for scripts and migrations.
- Use `-1` when the entire file must commit or roll back atomically.
- Never echo a connection string or enable shell tracing around a connection-string command.
- A rollback does not neutralize functions with external side effects. Inspect untrusted SQL before `EXPLAIN ANALYZE`.
