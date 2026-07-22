# HogQL cookbook

Use these patterns only when the official typed `query-*` tools cannot express the analysis. Before writing SQL against collected data, use the native `read-data-schema` tool to verify event names, properties, and relevant values in the active project.

Run a reviewed query with `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" hogql "<sql>"`, or use `hogql-file <path>` for multi-line input. Set and verify `POSTHOG_PROJECT_ID` first.

HogQL is ClickHouse SQL with PostHog-specific functions and pre-joined tables (`events`, `persons`, `sessions`, `session_replay_events`, `groups`, warehouse tables you've connected).

## Top events last 7 days

```sql
SELECT event, count() AS c
FROM events
WHERE timestamp >= now() - INTERVAL 7 DAY
GROUP BY event
ORDER BY c DESC
LIMIT 50
```

## DAU / WAU / MAU

```sql
SELECT
  uniq(distinct_id) FILTER (WHERE timestamp >= today())                               AS dau,
  uniq(distinct_id) FILTER (WHERE timestamp >= today() - INTERVAL 7 DAY)              AS wau,
  uniq(distinct_id) FILTER (WHERE timestamp >= today() - INTERVAL 30 DAY)             AS mau
FROM events
```

## Conversion funnel - signed up → activated → paid (last 30 days)

```sql
WITH per_user AS (
  SELECT
    distinct_id,
    countIf(event = 'user signed up')   > 0 AS signed_up,
    countIf(event = 'feature activated') > 0 AS activated,
    countIf(event = 'payment completed') > 0 AS paid
  FROM events
  WHERE timestamp >= now() - INTERVAL 30 DAY
  GROUP BY distinct_id
)
SELECT
  countIf(signed_up)                       AS step_1_signups,
  countIf(signed_up AND activated)         AS step_2_activations,
  countIf(signed_up AND activated AND paid) AS step_3_paid,
  round(100.0 * countIf(signed_up AND activated)         / nullIf(countIf(signed_up), 0), 2) AS conv_1_to_2_pct,
  round(100.0 * countIf(signed_up AND activated AND paid) / nullIf(countIf(signed_up AND activated), 0), 2) AS conv_2_to_3_pct
FROM per_user
```

For a richer funnel, use `query.kind = "FunnelsQuery"` via `posthog-query.sh raw`.

## Retention - week-over-week (cohort = users who fired `signed up` in week 0)

```sql
WITH cohort AS (
  SELECT distinct_id, min(toStartOfWeek(timestamp)) AS cohort_week
  FROM events
  WHERE event = 'user signed up' AND timestamp >= now() - INTERVAL 60 DAY
  GROUP BY distinct_id
),
returned AS (
  SELECT
    c.cohort_week,
    toStartOfWeek(e.timestamp) AS active_week,
    uniq(e.distinct_id)        AS users
  FROM cohort c
  JOIN events e ON e.distinct_id = c.distinct_id
  WHERE e.timestamp >= c.cohort_week
  GROUP BY c.cohort_week, active_week
)
SELECT cohort_week, active_week,
       dateDiff('week', cohort_week, active_week) AS week_offset,
       users
FROM returned
ORDER BY cohort_week, week_offset
```

## Top pages by unique users (web app)

```sql
SELECT
  properties.$current_url AS page,
  uniq(distinct_id)       AS unique_users,
  count()                 AS pageviews
FROM events
WHERE event = '$pageview' AND timestamp >= now() - INTERVAL 7 DAY
GROUP BY page
ORDER BY unique_users DESC
LIMIT 50
```

## Persons by property - list power users (>= 100 events)

```sql
SELECT
  person.id,
  person.properties.email AS email,
  count() AS events_count
FROM events
WHERE timestamp >= now() - INTERVAL 30 DAY
GROUP BY person.id, email
HAVING events_count >= 100
ORDER BY events_count DESC
LIMIT 100
```

## LLM cost rollup {#llm-cost-rollup}

PostHog's LLM observability writes one event per LLM call (typically `$ai_generation`). Cost-related properties: `$ai_total_cost_usd`, `$ai_input_tokens`, `$ai_output_tokens`, `$ai_model`, `$ai_provider`, `$ai_trace_id`.

Total spend last 30 days:
```sql
SELECT round(sum(toFloat(properties.$ai_total_cost_usd)), 2) AS total_usd
FROM events
WHERE event = '$ai_generation'
  AND timestamp >= now() - INTERVAL 30 DAY
```

By model:
```sql
SELECT
  properties.$ai_model AS model,
  count()                                                  AS calls,
  round(sum(toFloat(properties.$ai_total_cost_usd)), 2)    AS spend_usd,
  round(avg(toFloat(properties.$ai_total_cost_usd)), 4)    AS avg_per_call,
  sum(toInt(properties.$ai_input_tokens))                  AS input_tokens,
  sum(toInt(properties.$ai_output_tokens))                 AS output_tokens
FROM events
WHERE event = '$ai_generation'
  AND timestamp >= now() - INTERVAL 30 DAY
GROUP BY model
ORDER BY spend_usd DESC
```

By trace (worst offenders):
```sql
SELECT
  properties.$ai_trace_id AS trace_id,
  count() AS calls,
  round(sum(toFloat(properties.$ai_total_cost_usd)), 4) AS trace_cost
FROM events
WHERE event = '$ai_generation' AND timestamp >= now() - INTERVAL 7 DAY
GROUP BY trace_id
ORDER BY trace_cost DESC
LIMIT 20
```

## Error tracking - top issues by occurrences

```sql
SELECT
  properties.$exception_issue_id AS issue_id,
  any(properties.$exception_message) AS message,
  count() AS occurrences,
  uniq(distinct_id) AS affected_users
FROM events
WHERE event = '$exception' AND timestamp >= now() - INTERVAL 7 DAY
GROUP BY issue_id
ORDER BY occurrences DESC
LIMIT 25
```

For richer joins use `posthog-errors.sh ls` (queries the dedicated error_tracking endpoint), or `posthog-errors.sh query '<filter-json>'`.

## Session replays - sessions with errors

```sql
SELECT
  session_id,
  any(distinct_id)                  AS user,
  min(timestamp)                    AS started_at,
  countIf(event = '$exception')     AS errors,
  countIf(event = '$rageclick')     AS rage_clicks
FROM events
WHERE timestamp >= now() - INTERVAL 1 DAY
GROUP BY session_id
HAVING errors > 0
ORDER BY errors DESC
LIMIT 50
```

Generate a replay URL with `bash "$POSTHOG_SKILL_DIR/scripts/posthog-recordings.sh" url <session_id>`. Open it only when the user explicitly requested browser navigation.

## Feature flag exposure → conversion

```sql
WITH exposed AS (
  SELECT distinct_id, properties.$feature_flag_response AS variant
  FROM events
  WHERE event = '$feature_flag_called'
    AND properties.$feature_flag = 'new_pricing'
    AND timestamp >= now() - INTERVAL 14 DAY
),
converted AS (
  SELECT distinct_id
  FROM events
  WHERE event = 'subscription started' AND timestamp >= now() - INTERVAL 14 DAY
  GROUP BY distinct_id
)
SELECT
  e.variant,
  count(DISTINCT e.distinct_id)   AS exposed,
  count(DISTINCT c.distinct_id)   AS converted,
  round(100.0 * count(DISTINCT c.distinct_id) / nullIf(count(DISTINCT e.distinct_id), 0), 2) AS conv_pct
FROM exposed e
LEFT JOIN converted c USING (distinct_id)
GROUP BY e.variant
ORDER BY conv_pct DESC
```

## Joining a data warehouse table - Stripe revenue by signup cohort

Assumes you've connected Stripe via `$POSTHOG_SKILL_DIR/scripts/posthog-warehouse.sh source-create '...'` and the `stripe_charges` table is available.

```sql
WITH signups AS (
  SELECT distinct_id, min(toDate(timestamp)) AS signup_day
  FROM events
  WHERE event = 'user signed up'
  GROUP BY distinct_id
)
SELECT
  toStartOfMonth(s.signup_day) AS cohort_month,
  count(DISTINCT s.distinct_id) AS new_users,
  round(sum(c.amount_captured) / 100.0, 2) AS revenue_usd
FROM signups s
LEFT JOIN stripe_charges c ON c.metadata.distinct_id = s.distinct_id
GROUP BY cohort_month
ORDER BY cohort_month DESC
```

## Tips

- **Cap exploratory output with `LIMIT`** unless the task explicitly needs every row. Large and aborted queries still consume capacity.
- **`now() - INTERVAL N DAY`** is the canonical recency filter. `today()` is start-of-day UTC.
- **`person.properties.X`** reads the *current* property value at query time, not the historical value. For historical attribution use the value captured on the event (`properties.X`).
- **Do not assume `$`-prefixed events or properties exist.** Discover them in the active project's data schema first.
- **Validate first**: `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" validate "<sql>"` returns HogQL metadata and syntax diagnostics without running the analytical query.
- **Use async for long queries**: `bash "$POSTHOG_SKILL_DIR/scripts/posthog-query.sh" async "<sql>"` returns a `client_query_id`; poll it with `status` at a bounded cadence.
