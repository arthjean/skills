# Vercel REST API

Use this reference only when the native CLI does not cover the operation or when deterministic raw JSON is required. Prefer `bunx vercel@latest api` for endpoints in its current OpenAPI catalog, then use the bundled helper for remaining gaps.

**Base URL:** `https://api.vercel.com`
**Auth:** `Authorization: Bearer $VERCEL_TOKEN` on every request
**Team scoping:** add `?teamId=<id>` (or `?slug=<team-slug>`) on every team-scoped endpoint
**OpenAPI spec:** https://openapi.vercel.sh/
**Official docs:** https://vercel.com/docs/rest-api

## Invocation

```bash
VERCEL_SKILL_DIR="${VERCEL_SKILL_DIR:-$HOME/.agents/skills/vercel-cli}"

bunx vercel@latest api /v2/user
bunx vercel@latest api /v9/projects --scope <team> --paginate

bash "$VERCEL_SKILL_DIR/scripts/vercel-api.sh" GET "/v2/user"
bash "$VERCEL_SKILL_DIR/scripts/vercel-api.sh" GET "/v9/projects?limit=20"
bash "$VERCEL_SKILL_DIR/scripts/vercel-api.sh" POST "/v10/projects" \
  '{"name":"example-project"}'
```

`vercel-api.sh` adds bearer authentication, team scope when resolvable, JSON headers, one bounded retry, and JSON pretty-printing. Keep `VERCEL_TOKEN` in the environment.

## Endpoint categories

The OpenAPI spec is authoritative. Use this table only to find the likely family, then verify the current path before a mutation.

| Category | Typical coverage |
|---|---|
| Projects | CRUD, members, domains, environment variables, pause and transfer |
| Deployments | List, inspect, create, cancel, delete, events, and files |
| Environment | Typed variables, system variables, and bulk operations |
| Edge Config | Stores, items, tokens, backups, and read API |
| Webhooks and drains | Delivery configuration and lifecycle |
| Domains, registrar, DNS, aliases, certificates | Ownership, routing, and TLS |
| Teams and access groups | Membership, invitations, access, and billing scope |
| Logs and observability | Runtime logs, activity, metrics, traces, and usage |
| Security | Deployment protection, bypass tokens, OIDC, and password protection |
| Cache, redirects, routes, flags, rolling releases | Runtime behavior and traffic control |
| Marketplace, Connect, Sandbox | Integrations, networking, and compute resources |

## Most-used endpoints

### User & teams (no teamId scoping needed)

```http
GET /v2/user                       # current authenticated user
GET /v2/teams                      # list teams the user belongs to
GET /v2/teams/{id-or-slug}         # team details
```

### Projects

```http
GET    /v9/projects?teamId=...                       # list
GET    /v9/projects/{idOrName}?teamId=...            # find by id or name
POST   /v10/projects?teamId=...                      # create
PATCH  /v9/projects/{idOrName}?teamId=...            # update (rename, framework, autoExposeSystemEnvs, passwordProtection, etc.)
DELETE /v9/projects/{idOrName}?teamId=...            # delete
POST   /v1/projects/{idOrName}/pause?teamId=...      # pause builds
POST   /v1/projects/{idOrName}/unpause?teamId=...    # resume
GET    /v1/projects/{id}/members?teamId=...          # list members
POST   /v1/projects/{id}/members?teamId=...          # add member
DELETE /v1/projects/{id}/members/{uid}?teamId=...    # remove member
```

### Deployments

```http
GET    /v6/deployments?projectId=&teamId=&state=READY&target=production
GET    /v13/deployments/{id}?teamId=...              # full deployment details
POST   /v13/deployments?teamId=...                   # create (files[] or gitSource)
DELETE /v13/deployments/{id}?teamId=...              # delete
POST   /v12/deployments/{id}/cancel?teamId=...       # cancel in-progress
GET    /v3/deployments/{id}/events?teamId=...        # build + runtime events (paginated, supports follow=1 SSE)
GET    /v6/deployments/{id}/files?teamId=...         # list deployment files
GET    /v7/deployments/{id}/files/{fileId}?teamId=...# single file content
POST   /v13/deployments/{id}/promote?teamId=...      # promote to production
```

Response key fields: `uid`, `url`, `state` (`BUILDING`|`ERROR`|`READY`|`CANCELED`), `readyState`, `target`, `createdAt`, `buildingAt`, `ready`, `projectId`.

### Env vars

```http
GET    /v9/projects/{idOrName}/env?teamId=...        # list
POST   /v10/projects/{idOrName}/env?teamId=...       # create  body: {key, value, type, target[]}
PATCH  /v9/projects/{idOrName}/env/{id}?teamId=...   # update single
DELETE /v9/projects/{idOrName}/env/{id}?teamId=...   # delete
```

Body schema for create:
```json
{
  "key": "DATABASE_URL",
  "value": "postgres://...",
  "type": "encrypted | plain | sensitive",
  "target": ["production", "preview", "development"],
  "gitBranch": "main"
}
```

### Edge config

Management API (api.vercel.com):
```http
GET    /v1/edge-config?teamId=...
GET    /v1/edge-config/{id}?teamId=...
POST   /v1/edge-config?teamId=...                    # body: {slug}
DELETE /v1/edge-config/{id}?teamId=...
GET    /v1/edge-config/{id}/items?teamId=...
PATCH  /v1/edge-config/{id}/items?teamId=...         # bulk upsert  body: {items: [{operation, key, value}]}
GET    /v1/edge-config/{id}/item/{key}?teamId=...
GET    /v1/edge-config/{id}/token?teamId=...         # read tokens
```

Read API (high-volume reads, separate domain):
```http
GET https://edge-config.vercel.com/{id}/item/{key}
```

### Webhooks

```http
GET    /v1/webhooks?teamId=...
POST   /v1/webhooks?teamId=...                       # body: {url, events[], projectIds[]}
GET    /v1/webhooks/{id}?teamId=...
DELETE /v1/webhooks/{id}?teamId=...
```

Common events: `deployment.created`, `deployment.succeeded`, `deployment.error`, `deployment.canceled`, `deployment.ready`, `project.created`, `project.removed`, `domain.created`, `integration-configuration.removed`.

### Log drains

```http
GET    /v1/integrations/log-drains?teamId=...
POST   /v1/integrations/log-drains?teamId=...        # body: {url, deliveryFormat, headers, sources[]}
DELETE /v1/integrations/log-drains/{id}?teamId=...
```

Delivery formats: `json`, `ndjson`, `syslog`. Sources: `static`, `lambda`, `build`, `edge`, `external`.

### Domains

```http
GET    /v5/domains?teamId=...                        # list
POST   /v5/domains?teamId=...                        # add  body: {name}
GET    /v5/domains/{domain}?teamId=...
DELETE /v6/domains/{domain}?teamId=...
POST   /v5/domains/{domain}/verify?teamId=...        # trigger verification
GET    /v4/domains/{domain}/config?teamId=...        # propagation status
GET    /v4/domains/status?name={domain}              # availability check
GET    /v4/domains/price?name={domain}               # purchase price
```

### Aliases

```http
GET    /v4/aliases?projectId=&teamId=...
POST   /v2/deployments/{id}/aliases?teamId=...       # body: {alias}
DELETE /v2/aliases/{alias}?teamId=...
```

### Security / deployment protection

```http
POST   /v1/security/protection-bypass/{projectIdOrName}?teamId=...   # create bypass token
GET    /v1/security/protection-bypass/{projectIdOrName}?teamId=...   # list
DELETE /v1/security/protection-bypass/{projectIdOrName}/{tokenId}?teamId=...
```

To use a bypass secret in a request:
```bash
curl -fsSL -H "x-vercel-protection-bypass: $SECRET" "https://my-app-xyz.vercel.app/api/private"
# Or as query param:
curl -fsSL "https://my-app-xyz.vercel.app/api/private?_vercel_share=$SECRET"
```

Password protection: managed via `PATCH /v9/projects/{idOrName}` body field `passwordProtection`. Pro/Enterprise only.

### Cache (CDN + tag-based)

```http
POST /v1/edge-cache/purge?teamId=...                 # body: {projectId, type: "cdn"|"data"}
POST /v1/edge-cache/invalidate?teamId=...            # body: {projectId, tag}
```

## Polling async operations

Many writes (deployments, project deletes, domain transfers) are async. Pattern:

```bash
DEP_ID=$(vercel_api POST "/v13/deployments?teamId=$VERCEL_TEAM_ID" -H "Content-Type: application/json" -d "$BODY" | jq -r '.id')

while true; do
  STATE=$(vercel_api GET "/v13/deployments/$DEP_ID?teamId=$VERCEL_TEAM_ID" | jq -r '.readyState')
  case "$STATE" in
    READY)    echo "deployment ready"; break ;;
    ERROR|CANCELED) echo "failed: $STATE" >&2; exit 1 ;;
    *)        sleep 3 ;;
  esac
done
```

Prefer `bunx vercel@latest inspect <url> --wait` when CLI-side polling is sufficient.

## Rate limits

On 429 responses, honor `Retry-After` when present. The bundled helper retries once with a five-second delay. For bulk operations, batch and pace requests instead of adding an unbounded retry loop.

```bash
if ! response=$(vercel_api GET "/v9/projects" 2>&1); then
  if echo "$response" | grep -q '429'; then
    sleep 5
    response=$(vercel_api GET "/v9/projects")
  else
    echo "API error: $response" >&2
    exit 1
  fi
fi
```

## Token handling

Create tokens at https://vercel.com/account/tokens. Choose the narrowest scope and shortest lifetime compatible with the task. Store the token in `VERCEL_TOKEN`, never in a repository file or command argument. Revoke or rotate it through the account controls when exposure is suspected.
