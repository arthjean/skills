# PostHog REST API: helper fallback reference

This is the endpoint snapshot used by the bundled helpers. It is not a permanent API contract. Prefer the official agent API, and verify any raw path, method, scope, and body against current PostHog documentation before executing it.

Keep the user's project as the working directory and set:

```bash
POSTHOG_SKILL_DIR="${POSTHOG_SKILL_DIR:-$HOME/.agents/skills/posthog-cli}"
```

## Auth

```
Authorization: Bearer $POSTHOG_PERSONAL_API_KEY
Accept: application/json
Content-Type: application/json    # only for POST/PATCH
```

Personal API keys normally start with `phx_`. Project keys (`phc_*`) are public ingestion-only and cannot read management endpoints. Use the narrowest personal-key scopes that cover the requested operation.

## Base URLs

| Region | Host |
|---|---|
| US Cloud (default) | `https://us.posthog.com` |
| EU Cloud | `https://eu.posthog.com` |
| Self-hosted | `https://your-instance.example.com` |

## Generic curl pattern

```bash
curl -fsS \
  -H "Authorization: Bearer $POSTHOG_PERSONAL_API_KEY" \
  -H "Accept: application/json" \
  -H "Content-Type: application/json" \
  -X PATCH \
  -d '{"active":false}' \
  "https://us.posthog.com/api/projects/$PID/feature_flags/$FID/"
```

## Pagination

Many list endpoints return:
```json
{ "count": 1234, "next": "https://...?cursor=abc", "previous": null, "results": [...] }
```
Pagination style and default page size vary by endpoint. Follow `next` only when it stays on the authenticated PostHog origin. The helper refuses to forward credentials to a different origin.

```bash
url="$POSTHOG_HOST/api/projects/$PID/feature_flags/?limit=100"
while [[ -n "$url" && "$url" != "null" ]]; do
  resp=$(bash "$POSTHOG_SKILL_DIR/scripts/posthog-api.sh" GET "${url#"$POSTHOG_HOST"}")
  printf '%s\n' "$resp" | jq '.results[]'
  url=$(printf '%s\n' "$resp" | jq -r '.next')
done
```

## Rate limits

Limits differ by endpoint and can change. On HTTP 429, honor `Retry-After` when present. Use the official CLI's rate-limit controls for agent API calls, pace bulk helper workflows explicitly, and verify current limits before capacity planning.

## Error format

```json
{
  "type": "validation_error|authentication_error|server_error",
  "code": "specific_string_code",
  "detail": "Human-readable message",
  "attr": "field_name_if_applicable"
}
```

| Status | Meaning |
|---|---|
| 200 / 201 | success |
| 400 | validation error (`attr` identifies the field) |
| 401 | missing/invalid key |
| 403 | scope missing - re-issue the key with the right permission |
| 404 | not found |
| 429 | rate limited; retry after `Retry-After` seconds |
| 5xx | server error; retry with backoff |

## Endpoint catalog (project-scoped unless noted)

The helpers below currently use `/api/projects/{project_id}/` unless a path is explicitly organization- or user-scoped. Do not assume `/api/projects/` and `/api/environments/` are interchangeable for a new endpoint.

### Account

| Path | Methods | Notes |
|---|---|---|
| `/api/users/@me/` | GET, PATCH | current user, default org/team |
| `/api/users/@me/scene_personalisation/` | POST | dashboard layout prefs |

### Organizations & projects

| Path | Methods | Notes |
|---|---|---|
| `/api/organizations/` | GET | all orgs you can access |
| `/api/organizations/{org_id}/` | GET | single org metadata |
| `/api/organizations/{org_id}/members/` | GET | members |
| `/api/organizations/{org_id}/members/{member_id}/` | DELETE | remove member |
| `/api/organizations/{org_id}/projects/` | GET, POST | list/create projects in an org |
| `/api/organizations/{org_id}/roles/` | GET | custom roles |
| `/api/organizations/{org_id}/roles/{role_id}/` | GET | role detail |
| `/api/organizations/{org_id}/roles/{role_id}/role_memberships/` | GET | role members |
| `/api/organizations/{org_id}/activity_log/` | GET | org-level activity |
| `/api/projects/` | GET | all projects accessible across orgs |
| `/api/projects/{id}/` | GET, PATCH | project metadata + settings |

### HogQL / Query

| Path | Methods | Body |
|---|---|---|
| `/api/projects/{id}/query/` | POST | `{ "query": { "kind": "HogQLQuery", "query": "..." }, "name": "..." }` (sync) or add `"async": true` |
| `/api/projects/{id}/query/{client_query_id}/` | GET, DELETE | poll status / cancel |
| `/api/projects/{id}/query/{client_query_id}/log/` | GET | execution log (24h) |

Supported `query.kind` values:
- `HogQLQuery` - raw SQL
- `HogQLMetadata` - validation / introspection (`{"kind":"HogQLMetadata","language":"hogQL","query":"..."}`)
- `EventsQuery` - structured event log
- `PersonsNode` / `PersonsQuery` / `ActorsQuery`
- `TrendsQuery`, `FunnelsQuery`, `RetentionQuery`, `PathsQuery`, `StickinessQuery`, `LifecycleQuery`
- `SessionRecordingsQuery`
- `DataWarehouseQuery`
- `LogsQuery`, `ErrorTrackingQuery`
- `DatabaseSchemaQuery` - list available tables/columns

### Feature flags

| Path | Methods |
|---|---|
| `/api/projects/{id}/feature_flags/` | GET, POST |
| `/api/projects/{id}/feature_flags/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/feature_flags/{pk}/activity/` | GET |
| `/api/projects/{id}/feature_flags/{pk}/dependent_flags/` | GET |
| `/api/projects/{id}/feature_flags/{pk}/evaluation_reasons/` | GET |
| `/api/projects/{id}/feature_flags/{pk}/status/` | GET |
| `/api/projects/{id}/feature_flags/{pk}/user_blast_radius/` | POST |
| `/api/projects/{id}/feature_flags/copy_flags/` | POST |
| `/api/projects/{id}/feature_flags/local_evaluation/` | GET |

Flag create body:
```json
{
  "key": "my-flag",
  "name": "My flag",
  "active": true,
  "filters": {
    "groups": [{
      "properties": [{"key":"email","operator":"icontains","value":"@posthog.com","type":"person"}],
      "rollout_percentage": 100
    }],
    "multivariate": null,
    "payloads": {}
  }
}
```

### Experiments

| Path | Methods |
|---|---|
| `/api/projects/{id}/experiments/` | GET, POST |
| `/api/projects/{id}/experiments/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/experiments/{pk}/results/` | GET |
| `/api/projects/{id}/experiments/{pk}/stats/` | GET |
| `/api/projects/{id}/experiments/{pk}/timeseries/` | GET |
| `/api/projects/{id}/experiments/{pk}/duplicate/` | POST |
| `/api/projects/{id}/experiments/{pk}/reset/` | POST |
| `/api/projects/{id}/experiments/{pk}/create_exposure_cohort_for_experiment/` | POST |

Lifecycle is achieved by PATCHing `start_date` / `end_date` / `archived` / `conclusion` / `winning_variant`.

### Insights & dashboards

| Path | Methods |
|---|---|
| `/api/projects/{id}/insights/` | GET, POST |
| `/api/projects/{id}/insights/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/insights/{pk}/?refresh=blocking|async|force` | GET (run) |
| `/api/projects/{id}/insights/{pk}/sharing/` | GET |
| `/api/projects/{id}/insights/activity/` | GET |
| `/api/projects/{id}/insights/funnel/` | POST |
| `/api/projects/{id}/insights/retention/` | POST |
| `/api/projects/{id}/insights/trend/` | POST |
| `/api/projects/{id}/dashboards/` | GET, POST |
| `/api/projects/{id}/dashboards/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/dashboards/{pk}/?refresh=true` | GET |
| `/api/projects/{id}/dashboards/{pk}/sharing/` | GET |
| `/api/projects/{id}/dashboards/{pk}/move_tile/` | POST |
| `/api/projects/{id}/dashboards/{pk}/insights/` | POST | add tile |

### Cohorts

| Path | Methods |
|---|---|
| `/api/projects/{id}/cohorts/` | GET, POST |
| `/api/projects/{id}/cohorts/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/cohorts/{pk}/persons/` | GET, POST, DELETE | static cohorts only for POST/DELETE |
| `/api/projects/{id}/cohorts/{pk}/duplicate_as_static_cohort/` | POST |
| `/api/projects/{id}/cohorts/{pk}/activity/` | GET |

### Persons & events

| Path | Methods |
|---|---|
| `/api/projects/{id}/persons/` | GET |
| `/api/projects/{id}/persons/{pk}/` | GET, DELETE |
| `/api/projects/{id}/persons/{pk}/properties/` | GET |
| `/api/projects/{id}/persons/{pk}/update_property/` | POST `{"key":"plan","value":"pro"}` |
| `/api/projects/{id}/persons/{pk}/delete_property/` | POST `{"key":"plan"}` |
| `/api/projects/{id}/persons/{pk}/activity/` | GET |
| `/api/projects/{id}/persons/{pk}/cohorts/` | GET |
| `/api/projects/{id}/persons/values/` | GET `?key=plan` |
| `/api/projects/{id}/persons/bulk_delete/` | POST `{"distinct_ids":[...]}` or `{"ids":[...]}` |
| `/api/projects/{id}/events/` | GET - read-only, filter via query params |
| `/api/projects/{id}/events/values/` | GET `?key=$browser` |
| `/api/projects/{id}/event_definitions/` | GET |
| `/api/projects/{id}/event_definitions/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/property_definitions/` | GET |
| `/api/projects/{id}/property_definitions/{pk}/` | GET, PATCH, DELETE |

Events filter params: `?event=<name>&distinct_id=<id>&after=<iso>&before=<iso>&properties=[{"key":...,"value":...,"operator":...}]&search=<q>&limit=N`.

### Error tracking

**These endpoints REQUIRE the `/api/environments/{env_id}/` prefix** - the `/api/projects/{id}/` prefix returns 404. For PostHog accounts that predate the multi-environment feature (most), `env_id == project_id`. Confirmed empirically against EU Cloud, May 2026.

| Path | Methods |
|---|---|
| `/api/environments/{env_id}/error_tracking/issues/` | GET |
| `/api/environments/{env_id}/error_tracking/issues/{pk}/` | GET, PATCH |
| `/api/environments/{env_id}/error_tracking/issues/{pk}/events/` | GET |
| `/api/environments/{env_id}/error_tracking/issues/{pk}/merge/` | POST `{"ids":[...]}` |
| `/api/environments/{env_id}/error_tracking/issues/{pk}/split/` | POST `{"fingerprints":[...]}` |
| `/api/environments/{env_id}/error_tracking/grouping_rules/` | GET, POST |
| `/api/environments/{env_id}/error_tracking/suppression_rules/` | GET, POST |
| `/api/environments/{env_id}/error_tracking/assignment_rules/` | GET, POST |

Issue PATCH accepts `{"status":"resolved|active|suppressed", "assignee":{"type":"user","id":N}}`.

### Surveys, notebooks, recordings, annotations, actions

| Path | Methods |
|---|---|
| `/api/projects/{id}/surveys/` | GET, POST |
| `/api/projects/{id}/surveys/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/surveys/{pk}/stats/` | GET |
| `/api/projects/{id}/surveys/stats/` | GET | global stats |
| `/api/projects/{id}/surveys/responses_count/` | GET |
| `/api/projects/{id}/notebooks/` | GET, POST |
| `/api/projects/{id}/notebooks/{short_id}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/session_recordings/` | GET |
| `/api/projects/{id}/session_recordings/{recording_id}/` | GET, DELETE |
| `/api/projects/{id}/session_recordings/{recording_id}/snapshots/` | GET |
| `/api/projects/{id}/session_recordings/{recording_id}/summarize/` | POST |
| `/api/projects/{id}/session_recording_playlists/` | GET, POST |
| `/api/projects/{id}/session_recording_playlists/{pk}/` | GET, PATCH |
| `/api/projects/{id}/annotations/` | GET, POST |
| `/api/projects/{id}/annotations/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/actions/` | GET, POST |
| `/api/projects/{id}/actions/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/actions/{pk}/count/` | GET |
| `/api/projects/{id}/actions/{pk}/people/` | GET |

### Activity log

| Path | Methods |
|---|---|
| `/api/projects/{id}/activity_log/` | GET |
| `/api/projects/{id}/advanced_activity_logs/` | GET |
| `/api/projects/{id}/advanced_activity_logs/list/` | POST | filtered list |
| `/api/projects/{id}/advanced_activity_logs/filters/` | GET | available filters |

### Data warehouse

| Path | Methods |
|---|---|
| `/api/projects/{id}/external_data_sources/` | GET, POST |
| `/api/projects/{id}/external_data_sources/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/external_data_sources/{pk}/reload/` | POST |
| `/api/projects/{id}/external_data_sources/{pk}/jobs/` | GET |
| `/api/projects/{id}/external_data_sources/{pk}/check_cdc_prerequisites/` | POST |
| `/api/projects/{id}/external_data_sources/wizard/` | POST |
| `/api/projects/{id}/external_data_sources/db_schema/` | GET |
| `/api/projects/{id}/external_data_schemas/` | GET |
| `/api/projects/{id}/external_data_schemas/{pk}/` | GET, PATCH |
| `/api/projects/{id}/external_data_schemas/{pk}/cancel/` | POST |
| `/api/projects/{id}/external_data_schemas/{pk}/resync/` | POST |
| `/api/projects/{id}/external_data_schemas/{pk}/reload/` | POST |
| `/api/projects/{id}/warehouse_tables/` | GET |
| `/api/projects/{id}/warehouse_saved_queries/` | GET, POST |
| `/api/projects/{id}/warehouse_saved_queries/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/warehouse_saved_queries/{pk}/run/` | POST |
| `/api/projects/{id}/warehouse_saved_queries/{pk}/materialize/` | POST |
| `/api/projects/{id}/warehouse_saved_queries/{pk}/unmaterialize/` | POST |
| `/api/projects/{id}/data_warehouse_data_health_issues/` | GET |

### CDP - Hog functions & Hog flows

| Path | Methods |
|---|---|
| `/api/projects/{id}/hog_functions/` | GET, POST |
| `/api/projects/{id}/hog_functions/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/hog_functions/{pk}/logs/` | GET |
| `/api/projects/{id}/hog_functions/{pk}/metrics/` | GET |
| `/api/projects/{id}/hog_functions/{pk}/invocations/` | POST |
| `/api/projects/{id}/hog_function_templates/` | GET |
| `/api/projects/{id}/hog_function_templates/{pk}/` | GET |
| `/api/projects/{id}/hog_flows/{pk}/logs/` | GET |
| `/api/projects/{id}/hog_flows/{pk}/metrics/` | GET |

### LLM observability

| Path | Methods |
|---|---|
| `/api/projects/{id}/llm_observability/prompts/` | GET, POST |
| `/api/projects/{id}/llm_observability/prompts/{pk}/` | GET, PATCH |
| `/api/projects/{id}/llm_observability/prompts/{pk}/duplicate/` | POST |
| `/api/projects/{id}/llm_observability/evaluations/` | GET, POST |
| `/api/projects/{id}/llm_observability/evaluations/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/llm_observability/evaluations/{pk}/run/` | POST |
| `/api/projects/{id}/llm_observability/evaluations/judge_models/` | GET |
| `/api/projects/{id}/llm_observability/evaluations/test_hog/` | POST |
| `/api/projects/{id}/llm_observability/evaluation_config/` | GET |
| `/api/projects/{id}/llm_observability/evaluation_config/set_active_key/` | POST |
| `/api/projects/{id}/llm_observability/evaluation_reports/` | GET, POST |
| `/api/projects/{id}/llm_observability/evaluation_reports/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/llm_observability/evaluation_reports/{pk}/generate/` | POST |
| `/api/projects/{id}/llm_observability/evaluation_reports/{pk}/runs/` | GET |
| `/api/projects/{id}/llm_observability/sentiment/` | POST |
| `/api/projects/{id}/llm_observability/summarization/` | POST |
| `/api/projects/{id}/llm_observability/trace_reviews/` | GET, POST |
| `/api/projects/{id}/llm_observability/trace_reviews/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/llm_observability/review_queues/` | GET, POST |
| `/api/projects/{id}/llm_observability/review_queues/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/llm_observability/review_queues/{qid}/items/` | GET, POST |
| `/api/projects/{id}/llm_observability/review_queues/{qid}/items/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/llm_observability/skills/` | GET, POST |
| `/api/projects/{id}/llm_observability/skills/{pk}/` | GET |
| `/api/projects/{id}/llm_observability/skills/{pk}/duplicate/` | POST |
| `/api/projects/{id}/llm_observability/skills/{sid}/files/` | GET, POST |
| `/api/projects/{id}/llm_observability/skills/{sid}/files/{pk}/` | GET, DELETE |
| `/api/projects/{id}/llm_observability/skills/{sid}/files/{pk}/rename/` | POST |
| `/api/projects/{id}/llm_observability/clustering_jobs/` | GET |
| `/api/projects/{id}/llm_observability/clustering_jobs/{pk}/` | GET |

### Alerts, subscriptions, comments, integrations, scheduled changes, early access

| Path | Methods |
|---|---|
| `/api/projects/{id}/alerts/` | GET, POST |
| `/api/projects/{id}/alerts/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/alerts/{pk}/simulate/` | POST |
| `/api/projects/{id}/subscriptions/` | GET, POST |
| `/api/projects/{id}/subscriptions/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/subscriptions/{pk}/test_delivery/` | POST |
| `/api/projects/{id}/subscriptions/{pk}/deliveries/` | GET |
| `/api/projects/{id}/subscription_deliveries/` | GET |
| `/api/projects/{id}/comments/` | GET |
| `/api/projects/{id}/comments/{pk}/` | GET |
| `/api/projects/{id}/comments/{pk}/thread/` | GET |
| `/api/projects/{id}/comments/count/` | GET |
| `/api/projects/{id}/integrations/` | GET |
| `/api/projects/{id}/integrations/{pk}/` | GET, DELETE |
| `/api/projects/{id}/integrations/{pk}/channels/` | GET |
| `/api/projects/{id}/scheduled_changes/` | GET, POST |
| `/api/projects/{id}/scheduled_changes/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/early_access_features/` | GET, POST |
| `/api/projects/{id}/early_access_features/{pk}/` | GET, PATCH, DELETE |

### Inbox, change requests, approval policies, SDK doctor, web digest, usage metrics, proxy, SQL variables

| Path | Methods |
|---|---|
| `/api/projects/{id}/inbox_reports/` | GET |
| `/api/projects/{id}/inbox_reports/{pk}/` | GET |
| `/api/projects/{id}/inbox_source_configs/` | GET |
| `/api/projects/{id}/inbox_source_configs/{pk}/` | GET |
| `/api/projects/{id}/conversations/tickets/` | GET |
| `/api/projects/{id}/conversations/tickets/{pk}/` | GET, PATCH |
| `/api/projects/{id}/change_requests/` | GET |
| `/api/projects/{id}/change_requests/{pk}/` | GET |
| `/api/projects/{id}/approval_policies/` | GET |
| `/api/projects/{id}/approval_policies/{pk}/` | GET |
| `/api/projects/{id}/sdk_doctor/` | GET |
| `/api/projects/{id}/web_analytics/weekly_digest/` | GET |
| `/api/projects/{id}/usage_metrics/` | GET, POST |
| `/api/projects/{id}/usage_metrics/{pk}/` | GET, PATCH, DELETE |
| `/api/projects/{id}/proxy_records/` | GET, POST |
| `/api/projects/{id}/proxy_records/{pk}/` | GET, DELETE |
| `/api/projects/{id}/proxy_records/{pk}/retry/` | POST |
| `/api/projects/{id}/insight_variables/` | POST |
| `/api/projects/{id}/insight_variables/{pk}/` | PATCH, DELETE |

### Logs (PostHog product analytics logs / log explorer)

| Path | Methods |
|---|---|
| `/api/projects/{id}/logs/count/` | POST |
| `/api/projects/{id}/logs/sparkline/` | POST |
| `/api/projects/{id}/logs/attributes/` | POST |
| `/api/projects/{id}/logs/attribute_values/` | POST |
| `/api/projects/{id}/logs/count_ranges/` | POST |

## Notes on the `/api/environments/` migration

Some data-scoped APIs use `/api/environments/{environment_id}/`, while many management resources remain under `/api/projects/{project_id}/`. An environment ID is not automatically interchangeable with a project ID. Verify the exact current route before adding or changing a helper; never perform a global prefix replacement.
