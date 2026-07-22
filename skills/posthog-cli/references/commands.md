# Commands reference: `@posthog/cli` and helper scripts

Set the skill directory once while keeping the current working directory in the user's project:

```bash
POSTHOG_SKILL_DIR="${POSTHOG_SKILL_DIR:-$HOME/.agents/skills/posthog-cli}"
```

## Official `@posthog/cli`

Run the current package through Bun:

```bash
bunx --bun @posthog/cli@latest <command> ...
```

Do not install it globally. Current command families include:

| Subcommand | Purpose |
|---|---|
| `api` | Discover, inspect, validate, and call the agent-oriented PostHog tool catalog |
| `sourcemap` | Inject and upload JavaScript source maps |
| `dsym` | Upload Apple dSYM debug symbols |
| `hermes` | Upload Hermes source maps |
| `proguard` | Upload ProGuard mapping files |
| `symbol-sets` | Upload, download, and manage symbol sets |
| `exp` | Experimental commands; inspect current help before use |
| `login` | Interactive browser authentication that persists a local personal token |

### Agent API

```bash
bunx --bun @posthog/cli@latest api search '<narrow-query-or-regex>'
bunx --bun @posthog/cli@latest api info <tool>
bunx --bun @posthog/cli@latest api schema <tool> [field.path]
bunx --bun @posthog/cli@latest api call [--json] [--dry-run] [--confirm] <tool> '<json>'
```

Use `search` before the full `tools` inventory. Run `info` once when a schema is missing. If a field has a `hint`, drill into it with `schema` before constructing the value. Use `call --dry-run` before mutations and add `--confirm` only for an explicitly authorized destructive call against verified IDs.

`api skill list` is read-only. `api skill install` and `api agents-md install` modify local or repository configuration, so run them only when the user explicitly asks.

### Authentication

Prefer environment variables for headless Codex sessions:

```bash
export POSTHOG_CLI_API_KEY=phx_xxxxxxxxxxxxxxxxxxxx
export POSTHOG_CLI_PROJECT_ID=12345
export POSTHOG_CLI_HOST=https://us.posthog.com
```

Do not run `login` unless browser authentication and local token persistence are explicit. The helper scripts accept the CLI variables above plus the legacy `POSTHOG_PERSONAL_API_KEY`, `POSTHOG_PROJECT_ID`, and `POSTHOG_HOST` names.

### Artifact commands

Artifact command flags change more often than the REST helpers. Inspect current help instead of copying a stale upload form:

```bash
bunx --bun @posthog/cli@latest sourcemap --help
bunx --bun @posthog/cli@latest dsym --help
bunx --bun @posthog/cli@latest hermes --help
bunx --bun @posthog/cli@latest proguard --help
bunx --bun @posthog/cli@latest symbol-sets --help
```

The root command also exposes host, rate-limit, dotenv-file, and dry-run controls. Verify their current spelling with `bunx --bun @posthog/cli@latest --help` before use.

## Helper scripts - full subcommand surface

All helper scripts share these conventions:
- Last positional arg is always the optional `[project_id]`. If omitted, falls back to `$POSTHOG_PROJECT_ID`.
- Every command pretty-prints the JSON response via jq.
- Run any script without args to see its usage banner.

### `$POSTHOG_SKILL_DIR/scripts/posthog-ensure.sh`

Preflight. No subcommands. Verifies `bun`, `@posthog/cli` reachability, `jq`, `curl`, the API key (and prefix), the host, and runs a live `GET /api/users/@me/`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh`

Generic REST wrapper. `posthog-api.sh <METHOD> <PATH> [json_body | curl_extra_args]`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh`

`ls`, `ls-org <org_id>`, `get [project_id]`, `create <org_id> <name>`, `update <project_id> <patch-json>`, `switch <project_id>`, `me`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh`

`ls`, `get <org_id>`, `members <org_id>`, `rm-member <org_id> <member_id>`, `roles <org_id>`, `role <org_id> <role_id>`, `role-members <org_id> <role_id>`, `activity <org_id>`, `switch <org_id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-query.sh`

`hogql <sql>`, `hogql-file <path>`, `raw <body-json>`, `async <sql>`, `status <client_query_id>`, `log <client_query_id>`, `cancel <client_query_id>`, `schema`, `validate <sql>`, `table <sql>`, `logs <filter-json>`. Optional env: `POSTHOG_QUERY_NAME` to label queries.

### `$POSTHOG_SKILL_DIR/scripts/posthog-events.sh`

`ls`, `recent <event_name> [limit]`, `search <query>`, `defs [search] [type]`, `def-get <id>`, `def-rename <id> <new_name>`, `def-update <id> <patch>`, `props [search] [type] [group_index]`, `prop-get <id>`, `prop-update <id> <patch>`, `values <prop_key> [event] [limit]`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh`

`ls [limit]`, `get <person_id>`, `find <substring>`, `by-email <email>`, `by-distinct <distinct_id>`, `activity <person_id>`, `cohorts <person_id>`, `values <prop_key> [limit]`, `set-prop <person_id> <key> <value>`, `del-prop <person_id> <key>`, `rm <person_id>`, `bulk-rm <ids-csv>`, `bulk-rm-distinct <distinct_ids-csv>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh`

`ls`, `get <id>`, `create <name> <filters-json> [is_static]`, `create-static <name>`, `update <id> <patch>`, `rm <id>`, `persons <id>`, `add <id> <distinct_ids-csv>`, `remove <id> <distinct_ids-csv>`, `duplicate <id>`, `activity <id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh`

`ls`, `get <id|key>`, `by-key <key>`, `create <key> <name> [active] [rollout%]`, `create-json <body>`, `update <id> <patch>`, `enable <id>`, `disable <id>`, `rollout <id> <percent>`, `rm <id>`, `status <id>`, `dependents <id>`, `activity <id>`, `copy <id> <target_project_ids-csv>`, `blast <id> <conditions>`, `local-eval`, `schedule <id> <iso-date> <change-json>`, `schedules <id>`, `schedule-rm <id> <change_id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh`

`ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `launch <id>`, `pause <id>`, `resume <id>`, `end <id>`, `archive <id>`, `unarchive <id>`, `duplicate <id>`, `reset <id>`, `ship <id> <variant>`, `results <id>`, `stats <id>`, `timeseries <id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh`

`ls`, `search <query>`, `get <id>`, `by-short <short_id>`, `create <body>`, `update <id> <patch>`, `rename <id> <name>`, `rm <id>`, `run <id> [refresh]` (refresh: `blocking|async|force`), `sharing <id>`, `activity`, `url <id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh`

`ls`, `get <id>`, `create <name> [desc] [pinned]`, `create-json <body>`, `update <id> <patch>`, `rename <id> <name>`, `pin <id>`, `unpin <id>`, `rm <id>`, `refresh <id>`, `sharing <id>`, `add-tile <dashboard_id> <insight_id>`, `url <id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh`

`ls`, `get <issue_id>`, `update <id> <patch>`, `resolve <id>`, `ignore <id>`, `assign <id> <user_id>`, `merge <primary> <ids-csv>`, `split <id> <fingerprints-csv>`, `grouping-ls`, `grouping-add <body>`, `suppress-ls`, `suppress-add <body>`, `assign-ls`, `assign-add <body>`, `query <filter-json>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-surveys.sh`

`ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `launch <id>`, `stop <id>`, `archive <id>`, `rm <id>`, `stats <id>`, `global`, `activity`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-notebooks.sh`

`ls`, `get <short_id>`, `create <title> [content-json]`, `update <short_id> <patch>`, `rename <short_id> <new_title>`, `rm <short_id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-recordings.sh`

`ls [limit]`, `get <recording_id>`, `snapshots <id>`, `summarize <id>`, `rm <id>`, `playlists`, `playlist-get <id>`, `playlist-create <name>`, `playlist-update <id> <patch>`, `url <recording_id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-annotations.sh`

`ls`, `get <id>`, `create <content> [iso-date] [scope]` (scope: `project|dashboard_item|organization`), `update <id> <patch>`, `rm <id>`, `release <content> [version]`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-actions.sh`

`ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `count <id>`, `people <id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh`

Sources: `sources`, `source-get <id>`, `source-create <body>`, `source-update <id> <patch>`, `source-rm <id>`, `source-reload <id>`, `source-jobs <id>`, `source-schemas`.
Schemas: `schema-get <id>`, `schema-update <id> <patch>`, `schema-cancel <id>`, `schema-resync <id>`, `schema-reload <id>`.
Tables / views: `tables`, `views`, `view-get <id>`, `view-create <body>`, `view-update <id> <patch>`, `view-rm <id>`, `view-run <id>`, `view-materialize <id>`, `view-unmaterialize <id>`.
Health: `health`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh`

Hog functions: `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `enable <id>`, `disable <id>`, `rm <id>`, `logs <id>`, `metrics <id>`, `invoke <id> <event-json>`.
Templates: `templates`, `template-get <id>`.
Hog flows: `flow-logs <flow_id>`, `flow-metrics <flow_id>`.

### `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh`

Two-level dispatch: `<resource> <subcommand> [args...]`.

| Resource | Subcommands |
|---|---|
| `prompts` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `duplicate <id>` |
| `evaluations` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `run <id>`, `judge-models`, `test-hog <body>` |
| `eval-config` | `get`, `set-active <key>` |
| `reports` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `generate <id>`, `runs <id>` |
| `sentiment` | `<body-json>` |
| `summarize` | `<body-json>` |
| `reviews` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>` |
| `queues` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>` |
| `queue-items` | `ls <queue_id>`, `add <queue_id> <body>`, `get <queue_id> <item_id>`, `update <queue_id> <item_id> <patch>`, `rm <queue_id> <item_id>` |
| `skills` | `ls`, `get <id>`, `create <body>`, `duplicate <id>` |
| `skill-files` | `ls <skill_id>`, `add <skill_id> <body>`, `get <skill_id> <file_id>`, `rename <skill_id> <file_id> <new_name>`, `rm <skill_id> <file_id>` |
| `clusters` | `ls`, `get <job_id>` |

### `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh`

Two-level dispatch.

| Resource | Subcommands |
|---|---|
| `alerts` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `simulate <id>` |
| `subs` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>`, `test <id>`, `deliveries [sub_id]` |
| `comments` | `ls`, `get <id>`, `count`, `thread <comment_id>` |
| `integrations` | `ls`, `get <id>`, `rm <id>`, `channels <id>` |
| `scheduled` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>` |
| `early-access` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>` |
| `activity` | `project`, `advanced [filter-json]`, `filters` |
| `change-req` | `ls`, `get <id>` |
| `approval` | `ls`, `get <id>` |
| `inbox` | `reports-ls`, `reports-get <id>`, `sources-ls`, `sources-get <id>`, `tickets-ls`, `tickets-get <id>`, `tickets-update <id> <patch>` |
| `sdk-doctor` | `get` |
| `web-digest` | `get` |
| `usage` | `ls`, `get <id>`, `create <body>`, `update <id> <patch>`, `rm <id>` |
| `proxy` | `ls`, `get <id>`, `create <body>`, `rm <id>`, `retry <id>` |
| `sql-vars` | `create <body>`, `update <id> <patch>`, `rm <id>` |
| `debug-mcp` | `ui-apps` |
| `logs` | `count <body>`, `sparkline <body>`, `attrs <body>`, `attr-values <body>`, `count-ranges <body>` |

## Common bash patterns

### Resolve current user's default project + auto-export

```bash
me=$(bash "$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh" me)
export POSTHOG_PROJECT_ID="$(printf '%s' "$me" | jq -r '.team.id')"
printf 'Active project: %s (%s)\n' "$(printf '%s' "$me" | jq -r '.team.name')" "$POSTHOG_PROJECT_ID"
```

### Pipe HogQL TSV into a CSV

```bash
bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" table "
  SELECT distinct_id, properties.email, count() AS events
  FROM events
  WHERE timestamp >= now() - INTERVAL 30 DAY
  GROUP BY distinct_id, properties.email
" | tr '\t' ',' > active-users.csv
```

### Bulk-disable feature flags matching a prefix

```bash
targets=$(mktemp)
bash "$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh" ls \
  | jq -r '.results[] | select(.key | startswith("legacy_")) | .id' > "$targets"

wc -l "$targets"
head "$targets"

# Continue only after reviewing the IDs and receiving explicit authorization.
while IFS= read -r fid; do
  bash "$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh" disable "$fid"
  sleep 0.05
done < "$targets"
rm -f "$targets"
```

### Move all "active" error tracking issues older than 30 days into "suppressed"

```bash
cutoff='<reviewed ISO-8601 timestamp>'
targets=$(mktemp)
bash "$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh" ls \
  | jq -r --arg cutoff "$cutoff" \
      '.results[] | select(.status=="active" and .last_seen < $cutoff) | .id' > "$targets"

wc -l "$targets"
head "$targets"

# Continue only after reviewing the issues and receiving explicit authorization.
while IFS= read -r iid; do
  bash "$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh" ignore "$iid"
done < "$targets"
rm -f "$targets"
```
