# Neon Management API through neonctl

Use the authenticated `neonctl api` passthrough for routes without a dedicated CLI command. It avoids duplicating auth headers and can discover routes from the current OpenAPI description.

## Discover routes

```bash
bunx neonctl@latest api --list
bunx neonctl@latest api --list --refresh
```

Refresh only when the cached route list appears stale.

## GET

```bash
bunx neonctl@latest api "/projects/$PID" --output json

bunx neonctl@latest api \
  "/projects/$PID/endpoints" \
  --output json

bunx neonctl@latest api \
  "/projects/$PID/operations/$OP_ID" \
  --output json
```

## POST and PATCH

Use typed fields for small request bodies:

```bash
bunx neonctl@latest api \
  "/projects/$PID/branches" \
  --method POST \
  --field branch.name=dev \
  --output json

bunx neonctl@latest api \
  "/projects/$PID/endpoints/$ENDPOINT_ID" \
  --method PATCH \
  --field endpoint.autoscaling_limit_min_cu=0.25 \
  --field endpoint.autoscaling_limit_max_cu=4 \
  --field endpoint.suspend_timeout_seconds=300 \
  --output json
```

Use raw JSON for complex bodies:

```bash
bunx neonctl@latest api \
  "/projects/$PID/endpoints/$ENDPOINT_ID" \
  --method PATCH \
  --data @request.json \
  --output json
```

Do not interpolate untrusted JSON into a shell string. Put it in a reviewed file or pass typed `--field` values.

## Query parameters and headers

```bash
bunx neonctl@latest api \
  "/projects/$PID/operations" \
  --query limit=20 \
  --include \
  --output json
```

Do not add an `Authorization` header manually. `neonctl` uses `NEON_API_KEY`.

## Poll an operation

First inspect the actual create response and extract its operation ID. Then poll the dedicated route:

```bash
while true; do
  RESPONSE="$(
    bunx neonctl@latest api \
      "/projects/$PID/operations/$OP_ID" \
      --output json
  )"
  STATUS="$(printf '%s\n' "$RESPONSE" | jq -r '.operation.status')"

  case "$STATUS" in
    finished)
      break
      ;;
    failed)
      printf 'Neon operation failed\n' >&2
      exit 1
      ;;
    *)
      sleep 1
      ;;
  esac
done
```

Bound polling in production automation with a deadline and exponential backoff. Do not wait indefinitely.

## SQL is separate

Do not assume an undocumented Management API route can execute arbitrary SQL. Use `neonctl psql`, the bundled helpers, or the supported Neon serverless driver for runtime application code.

## Key scope

Use the narrowest API-key scope that can perform the requested operation. Keep `NEON_API_KEY` in the environment, never in a request body, committed file, command argument, or response.
