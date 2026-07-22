# Legacy MCP to PostHog CLI and helper parity

The current official CLI exposes the PostHog agent tool catalog directly. Strip any `mcp__posthog__` or `posthog:` namespace from a legacy tool name, then inspect and call the bare tool through the native agent API:

```bash
bunx --bun @posthog/cli@latest api info <tool-name>
bunx --bun @posthog/cli@latest api call --json <tool-name> '<json-input>'
```

If the bare name no longer exists, use `api search <narrow-pattern>` instead of guessing a rename. The tables below are a legacy map to bundled Bash helpers, useful only when a deterministic shell pipeline or REST gap justifies bypassing the native agent API. Bold rows indicate helper-only convenience behavior; MCP-gap notes are historical and must be rechecked against the current CLI catalog.

### Account, orgs, projects

| MCP tool | bash equivalent | Notes |
|---|---|---|
| `user-get` / `user-home-settings-get` / `user-home-settings-update` / `user-settings-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh me` + `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh PATCH /api/users/@me/ '{...}'` | Direct REST for less-common patches. |
| `organizations-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh ls` | |
| `organization-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh get <id>` | |
| `org-members-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh members <id>` | |
| `roles-list` / `role-get` / `role-members-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh roles|role|role-members` | |
| `switch-organization` | `$POSTHOG_SKILL_DIR/scripts/posthog-orgs.sh switch <id>` (prints export hint) | |
| `project-get` / `projects-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh get` / `ls` | |
| `project-settings-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh update <id> '<patch>'` | |
| `switch-project` | `$POSTHOG_SKILL_DIR/scripts/posthog-projects.sh switch <id>` (prints export hint) | |

### HogQL & queries

| MCP tool | bash equivalent | Notes |
|---|---|---|
| `query-run` | `$POSTHOG_SKILL_DIR/scripts/posthog-query.sh hogql "<sql>"` or `raw <body>` | |
| `query-validate` | `$POSTHOG_SKILL_DIR/scripts/posthog-query.sh validate "<sql>"` | |
| `query-generate-hogql-from-question` | *(not implemented in bash - uses LLM tooling MCP-side)* | Use Codex to generate HogQL, then run via `hogql`. |
| `query-logs` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh logs sparkline '<body>'` etc. | |
| **`hogql-schema`** | `$POSTHOG_SKILL_DIR/scripts/posthog-query.sh schema` | Returns full `DatabaseSchemaQuery` result; richer than MCP's filtered output. |
| `query-error-tracking-issues` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh query '<filter-json>'` | |
| `get-llm-total-costs-for-project` | HogQL recipe → see [hogql-cookbook.md](hogql-cookbook.md#llm-cost-rollup) | |

### Feature flags

| MCP tool | bash equivalent | Notes |
|---|---|---|
| `feature-flag-get-all` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh ls` | |
| `feature-flag-get-definition` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh get <id>` / `by-key <key>` | |
| `create-feature-flag` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh create <key> <name>` (simple) or `create-json '<body>'` (full) | |
| `update-feature-flag` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh update <id> '<patch>'` / `enable` / `disable` / `rollout` | Convenience subcommands beyond MCP. |
| `delete-feature-flag` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh rm <id>` | |
| `feature-flags-status-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh status <id>` | |
| `feature-flags-dependent-flags-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh dependents <id>` | |
| `feature-flags-evaluation-reasons-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh GET /api/projects/$PID/feature_flags/$ID/evaluation_reasons/` | |
| `feature-flags-activity-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh activity <id>` | |
| `feature-flags-copy-flags-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh copy <id> "<targets-csv>"` | |
| `feature-flags-user-blast-radius-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh blast <id> '<conditions>'` | |

### Experiments

| MCP tool | bash equivalent |
|---|---|
| `experiment-list` / `experiment-get-all` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh ls` |
| `experiment-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh get <id>` |
| `experiment-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh create '<body>'` |
| `experiment-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh update <id> '<patch>'` |
| `experiment-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh rm <id>` |
| `experiment-launch` / `pause` / `resume` / `end` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh launch|pause|resume|end <id>` |
| `experiment-archive` / `unarchive` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh archive|unarchive <id>` |
| `experiment-duplicate` / `reset` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh duplicate|reset <id>` |
| `experiment-ship-variant` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh ship <id> <variant>` |
| `experiment-results-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh results <id>` |
| `experiment-stats` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh stats <id>` |
| `experiment-timeseries-results` | `$POSTHOG_SKILL_DIR/scripts/posthog-experiments.sh timeseries <id>` |

### Insights & dashboards

| MCP tool | bash equivalent |
|---|---|
| `insights-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh ls` |
| `insight-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh get <id>` |
| `insight-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh create '<body>'` |
| `insight-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh update <id> '<patch>'` |
| `insight-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh rm <id>` |
| `insight-query` | `$POSTHOG_SKILL_DIR/scripts/posthog-insights.sh run <id>` |
| `dashboards-get-all` | `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh ls` |
| `dashboard-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh get <id>` |
| `dashboard-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh create <name>` / `create-json '<body>'` |
| `dashboard-update` / `dashboard-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh update|rm` |
| `dashboard-reorder-tiles` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh POST /api/projects/$PID/dashboards/$DID/move_tile/ '<body>'` |
| `dashboard-insights-run` | `$POSTHOG_SKILL_DIR/scripts/posthog-dashboards.sh refresh <id>` |

### Cohorts, persons, events

| MCP tool | bash equivalent |
|---|---|
| `cohorts-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh ls` |
| `cohorts-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh get <id>` |
| `cohorts-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh create <name> '<filters>'` / `create-static <name>` |
| `cohorts-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh update <id> '<patch>'` |
| `cohorts-add-persons-to-static-cohort-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh add <id> "csv"` |
| `cohorts-rm-person-from-static-cohort-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-cohorts.sh remove <id> "csv"` |
| `persons-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh ls` / `find` / `by-email` / `by-distinct` |
| `persons-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh get <id>` |
| `persons-cohorts-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh cohorts <id>` |
| `persons-values-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh values <key>` |
| `persons-property-set` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh set-prop <id> <key> <val>` |
| `persons-property-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh del-prop <id> <key>` |
| `persons-bulk-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-persons.sh bulk-rm "ids"` / `bulk-rm-distinct "csv"` |
| `event-definitions-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-events.sh defs` |
| `event-definition-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-events.sh def-update <id> '<patch>'` / `def-rename <id> <name>` |
| `properties-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-events.sh props` |
| `entity-search` | grep across `$POSTHOG_SKILL_DIR/scripts/posthog-events.sh defs`, `props`, `posthog-actions.sh ls`, `posthog-flags.sh ls` |

### Error tracking

| MCP tool | bash equivalent |
|---|---|
| `error-tracking-issues-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh ls` |
| `error-tracking-issues-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh get <id>` |
| `error-tracking-issues-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh update|resolve|ignore|assign` |
| `error-tracking-issues-merge-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh merge <primary> "csv"` |
| `error-tracking-issues-split-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh split <id> "fingerprints-csv"` |
| `error-tracking-grouping-rules-list` / `-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh grouping-ls|grouping-add` |
| `error-tracking-suppression-rules-list` / `-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh suppress-ls|suppress-add` |
| `error-tracking-assignment-rules-list` / `-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh assign-ls|assign-add` |

### Surveys, notebooks, recordings, annotations, actions

| MCP tool | bash equivalent |
|---|---|
| `surveys-get-all` / `survey-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-surveys.sh ls|get|create|update|rm` |
| `survey-stats` / `surveys-global-stats` | `$POSTHOG_SKILL_DIR/scripts/posthog-surveys.sh stats <id>` / `global` |
| `notebooks-list` / `-retrieve` / `-create` / `-partial-update` / `-destroy` | `$POSTHOG_SKILL_DIR/scripts/posthog-notebooks.sh ls|get|create|update|rm` |
| `session-recording-get` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-recordings.sh get|rm` |
| `session-recording-summarize` | `$POSTHOG_SKILL_DIR/scripts/posthog-recordings.sh summarize <id>` |
| `session-recording-playlist-*` / `session-recording-playlists-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-recordings.sh playlists|playlist-get|playlist-create|playlist-update` |
| `annotations-list` / `annotation-retrieve` / `-create` / `annotations-partial-update` / `annotation-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-annotations.sh ls|get|create|update|rm` |
| `actions-get-all` / `action-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-actions.sh ls|get|create|update|rm` |

### Data warehouse

| MCP tool | bash equivalent |
|---|---|
| `external-data-sources-list` / `-retrieve` / `-create` / `-partial-update` / `-destroy` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh sources|source-get|source-create|source-update|source-rm` |
| `external-data-sources-reload` / `-jobs` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh source-reload|source-jobs` |
| `external-data-sources-check-cdc-prerequisites-create` / `-wizard` / `-db-schema` / `-refresh-schemas` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh POST /api/projects/$PID/external_data_sources/check_cdc_prerequisites/` etc. |
| `external-data-sources-create-webhook-create` / `-delete-webhook-create` / `-update-webhook-inputs-create` / `-webhook-info-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh ...` (REST direct) |
| `external-data-schemas-list` / `-retrieve` / `-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh source-schemas|schema-get|schema-update` |
| `external-data-schemas-cancel` / `-resync` / `-reload` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh schema-cancel|schema-resync|schema-reload` |
| `external-data-schemas-incremental-fields-create` / `-delete-data` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh POST/DELETE ...` |
| `external-data-sync-logs` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh GET /api/projects/$PID/external_data_sources/$ID/jobs/?include_logs=true` |
| `view-list` / `-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh views|view-get|view-create|view-update|view-rm` |
| `view-run` / `view-run-history` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh view-run` / `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh GET ../run_history/` |
| `view-materialize` / `-unmaterialize` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh view-materialize|view-unmaterialize` |
| `data-warehouse-data-health-issues-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh health` |

### CDP - Hog functions & Hog flows

| MCP tool | bash equivalent |
|---|---|
| `cdp-functions-list` / `-create` / `-retrieve` / `-partial-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh ls|create|get|update|rm` |
| `cdp-functions-logs-retrieve` / `-metrics-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh logs|metrics <id>` |
| `cdp-functions-invocations-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh invoke <id> '<event>'` |
| `cdp-functions-rearrange-partial-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh PATCH /api/projects/$PID/hog_functions/rearrange/ '<body>'` |
| `cdp-function-templates-list` / `-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh templates|template-get` |
| `hog-flows-logs-retrieve` / `-metrics-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-cdp.sh flow-logs|flow-metrics <id>` |

### LLM observability (`llma-*`)

| MCP tool | bash equivalent |
|---|---|
| `llma-prompt-list` / `-get` / `-create` / `-update` / `-duplicate` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh prompts ls|get|create|update|duplicate` |
| `llma-evaluation-list` / `-create` / `-get` / `-update` / `-delete` / `-run` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh evaluations ls|get|create|update|rm|run` |
| `llma-evaluation-judge-models` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh evaluations judge-models` |
| `llma-evaluation-test-hog` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh evaluations test-hog '<body>'` |
| `llma-evaluation-config-get` / `-set-active-key` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh eval-config get|set-active <key>` |
| `llma-evaluation-report-list` / `-get` / `-create` / `-update` / `-delete` / `-generate` / `-run-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh reports ls|get|create|update|rm|generate|runs` |
| `llma-evaluation-summary-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh summarize '<body>'` |
| `llma-sentiment-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh sentiment '<body>'` |
| `llma-summarization-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh summarize '<body>'` |
| `llma-trace-review-list` / `-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh reviews ls|get|create|update|rm` |
| `llma-review-queue-list` / `-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh queues ls|get|create|update|rm` |
| `llma-review-queue-item-list` / `-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh queue-items ls|get|add|update|rm` |
| `llma-skill-list` / `-get` / `-create` / `-update` / `-duplicate` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh skills ls|get|create|duplicate` |
| `llma-skill-file-create` / `-get` / `-rename` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh skill-files add|get|rename|rm` |
| `llma-clustering-job-list` / `-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-llm.sh clusters ls|get` |
| `get-llm-total-costs-for-project` | HogQL recipe - see [hogql-cookbook.md](hogql-cookbook.md#llm-cost-rollup) |

### Alerts, subscriptions, comments, integrations, scheduled changes, early access, inbox, change requests, approval policies, SDK doctor, web digest, usage metrics, proxy, SQL variables, logs, debug

| MCP tool | bash equivalent |
|---|---|
| `alerts-list` / `alert-get` / `-create` / `-update` / `-delete` / `-simulate` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh alerts ls|get|create|update|rm|simulate` |
| `subscriptions-list` / `-retrieve` / `-create` / `-partial-update` / `subscriptions-test-delivery-create` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh subs ls|get|create|update|rm|test` |
| `subscriptions-deliveries-list` / `-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh subs deliveries [sub_id]` |
| `comments-list` / `comment-get` / `-thread` / `-count` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh comments ls|get|thread|count` |
| `integrations-list` / `integration-get` / `-delete` / `integrations-channels-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh integrations ls|get|rm|channels` |
| `scheduled-changes-list` / `-get` / `-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh scheduled ls|get|create|update|rm` |
| `early-access-feature-list` / `-retrieve` / `-create` / `-partial-update` / `-destroy` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh early-access ls|get|create|update|rm` |
| `activity-log-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh activity project` |
| `advanced-activity-logs-list` / `-filters` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh activity advanced` / `filters` |
| `change-request-get` / `change-requests-list` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh change-req ls|get` |
| `approval-policies-list` / `approval-policy-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh approval ls|get` |
| `inbox-reports-list` / `-retrieve` / `inbox-source-configs-list` / `-retrieve` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh inbox reports-ls|reports-get|sources-ls|sources-get` |
| `conversations-tickets-list` / `-retrieve` / `-update` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh inbox tickets-ls|tickets-get|tickets-update` |
| `sdk-doctor-get` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh sdk-doctor get` |
| `web-analytics-weekly-digest` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh web-digest get` |
| `usage-metrics-list` / `-retrieve` / `-create` / `-partial-update` / `-destroy` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh usage ls|get|create|update|rm` |
| `proxy-list` / `proxy-get` / `-create` / `-delete` / `-retry` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh proxy ls|get|create|rm|retry` |
| `sql-variables-create` / `-update` / `-delete` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh sql-vars create|update|rm` |
| `logs-count` / `-sparkline-query` / `-attributes-list` / `-attribute-values-list` / `-count-ranges` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh logs count|sparkline|attrs|attr-values|count-ranges` |
| `debug-mcp-ui-apps` | `$POSTHOG_SKILL_DIR/scripts/posthog-misc.sh debug-mcp ui-apps` |
| `endpoint-*` (custom HogQL endpoints) | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh ... /api/projects/$PID/query_endpoints/...` (REST direct) |

### Endpoint helpers (custom HogQL endpoints / "Insight endpoints")

| MCP tool | bash equivalent |
|---|---|
| `endpoints-get-all` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh GET "/api/projects/$PID/query_endpoints/"` |
| `endpoint-get` / `endpoint-create` / `endpoint-update` / `endpoint-delete` / `endpoint-run` | `$POSTHOG_SKILL_DIR/scripts/posthog-api.sh GET\|POST\|PATCH\|DELETE "/api/projects/$PID/query_endpoints/[$ID/[run/]]"` |
| `endpoint-versions` / `endpoint-materialization-status` / `endpoint-openapi-spec` | direct REST |

### MCP-only / docs / search

| MCP tool | bash equivalent |
|---|---|
| `docs-search` | Use `bunx --bun @posthog/cli@latest api info docs-search`, then call it through the native agent API. |

## Beyond the MCP

These resources/operations are not exposed as MCP tools but ARE available via REST and through this skill:

| Resource | bash entrypoint | API path |
|---|---|---|
| **Logs explorer** queries (count/sparkline/attrs) | `posthog-misc.sh logs *` | `/api/projects/{id}/logs/*` |
| **Inbox tickets** patch | `posthog-misc.sh inbox tickets-update` | `/api/projects/{id}/conversations/tickets/{id}/` |
| **Project switching for the shell** | `posthog-projects.sh switch` | client-side |
| **HogQL TSV output** | `posthog-query.sh table` | client-side |
| **Annotations release helper** | `posthog-annotations.sh release` | client-side |
| **Flag rollout %** convenience | `posthog-flags.sh rollout` | derives PATCH body |
| **Flag bulk schedule** | `posthog-flags.sh schedule` | `/api/projects/{id}/scheduled_changes/` |
| **All endpoints not yet helper-wrapped** | `posthog-api.sh <METHOD> <PATH> [body]` | universal escape hatch |

## Workflow comparison

### MCP recipe: "find feature flags about checkout, disable the disabled-for-eu one"

```
mcp__posthog__feature-flag-get-all() → filter results in client → mcp__posthog__update-feature-flag(id, {active: false})
```

### bash equivalent

```bash
fid=$(bash "$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh" ls | jq -r '.results[] | select(.key|test("checkout"; "i")) | select(.key|contains("eu")) | .id' | head -1)
printf 'Review target feature flag ID: %s\n' "$fid"
# Execute only after the target and mutation are explicitly authorized.
bash "$POSTHOG_SKILL_DIR/scripts/posthog-flags.sh" disable "$fid"
```

### MCP recipe: "show me top errors this week and resolve the noise"

```
mcp__posthog__error-tracking-issues-list({status:"active"}) → filter → mcp__posthog__error-tracking-issues-partial-update(id, {status:"resolved"})
```

### bash equivalent

```bash
bash "$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh" ls | jq '.results | sort_by(.aggregations.occurrences) | reverse | .[0:5]'
# Execute only after inspecting the issue and confirming the requested state change.
bash "$POSTHOG_SKILL_DIR/scripts/posthog-errors.sh" resolve <issue_id>
```

## Execution model comparison

| | Native agent API | Bundled helpers |
|---|---|---|
| Auth | `POSTHOG_CLI_API_KEY` and CLI project context | CLI variables or legacy helper aliases, optionally loaded from project env files |
| Schema source | Current `api info` and `api schema` | Static wrapper contract plus current REST documentation |
| Mutation protection | `--dry-run`, with `--confirm` required for destructive tools | Caller must inspect and authorize the target before invocation |
| Best use | Default PostHog operations and typed analytics | REST gaps, reproducible shell pipelines, HogQL files, tabular output |
| Project context | CLI environment or tool input | Explicit argument, environment, then nearest project `.env.local` or `.env` |
