# Neon MCP to CLI workflow map

Use this map when translating an existing Neon MCP workflow. The MCP tool inventory and `neonctl` evolve independently, so verify current official documentation when an exact tool name or capability matters.

| MCP intent | CLI or helper |
|---|---|
| List projects | `bunx neonctl@latest projects list --output json` |
| Describe a project | `bunx neonctl@latest projects get "$PID" --output json` |
| Create a project | `bunx neonctl@latest projects create ... --output json` |
| Delete a project | `bunx neonctl@latest projects delete "$PID"` |
| List organizations | `bunx neonctl@latest orgs list --output json` |
| Create a branch | `bunx neonctl@latest branches create ... --output json` |
| List or describe branches | `branches list` or `branches get` |
| Delete a branch | `bunx neonctl@latest branches delete ...` |
| Reset from parent | `bunx neonctl@latest branches reset ... --parent` |
| Restore from a point in time | `bunx neonctl@latest branches restore ...` |
| Compare schemas | `bunx neonctl@latest branches schema-diff ...` |
| Get a connection string | `bunx neonctl@latest cs ...` |
| Run SQL | `scripts/neon-sql.sh` |
| Run a SQL transaction | `scripts/neon-tx.sh` |
| List tables | `scripts/neon-tables.sh` |
| Describe a table | `scripts/neon-describe.sh` |
| Explain SQL | `scripts/neon-explain.sh` |
| List slow queries | `scripts/neon-slow-queries.sh` |
| List computes | `bunx neonctl@latest api "/projects/$PID/endpoints"` |
| Get one operation | `bunx neonctl@latest api "/projects/$PID/operations/$OP_ID"` |
| Other Management API route | `bunx neonctl@latest api --list` then `api <path>` |
| Search or fetch Neon docs | Context7 CLI or official Neon documentation |

## Migration translation

An MCP migration pair usually maps to:

1. Create a schema-only preview branch.
2. Apply the migration there through `neon-tx.sh`.
3. Compare it with `branches schema-diff`.
4. Apply the reviewed migration to the target branch.
5. Preserve or remove the preview branch according to the rollback requirement.

Do not collapse steps 3 and 4. The schema diff is the review boundary.

## Query tuning translation

1. Create an isolated branch from the production-like source branch.
2. Capture an `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` baseline.
3. Apply index or query changes on the isolated branch.
4. Capture the new plan and compare execution time, row estimates, buffers, and scan types.
5. Apply only the reviewed change to the target branch.

`EXPLAIN ANALYZE` executes statements. Use read-only queries or the helper's `--safe` transaction wrapper for mutating SQL.

## Capability gaps

When no dedicated command exists:

1. Run `bunx neonctl@latest api --list`.
2. Identify the supported Management API route.
3. Use `neonctl api` with typed fields or a reviewed JSON file.
4. Do not invent an endpoint from an older MCP implementation.

Provisioning product features such as Neon Auth or Data API may now have dedicated CLI commands. Inspect current `neonctl --help` and official docs instead of relying on an old MCP parity table.
