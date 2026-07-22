# Resend REST API - endpoint catalog

Use this catalog only for a raw REST operation that the current official CLI does not cover. It was imported from a May 2026 API snapshot, so verify unfamiliar paths against [current Resend API documentation](https://resend.com/docs/api-reference/introduction) before executing them. The base URL is `https://api.resend.com`.

## Auth

Keep `RESEND_API_KEY` in the environment and use the bundled wrapper. It places authorization in a curl config file descriptor instead of a process argument:

```bash
bash "$RESEND_SKILL_DIR/scripts/resend-api.sh" GET /domains
```

The wrapper also sends `Accept: application/json` and a non-empty `User-Agent`.

Key formats: `re_*`. Two permission tiers:

| Permission        | What it can do                                                                 |
|-------------------|--------------------------------------------------------------------------------|
| `full_access`     | Manage every resource (domains, contacts, broadcasts, templates, webhooks, automations, api-keys). |
| `sending_access`  | Send emails only. Can be restricted to a single `domain_id` at creation time.  |

No granular per-resource scopes. No key rotation - delete and recreate.

## Rate limits

The documented default is 5 requests per second per **team** across all keys. Responses can carry:

```
ratelimit-limit: 5
ratelimit-remaining: 4
ratelimit-reset: 1
```

On 429, response carries `Retry-After: <seconds>`. Daily/monthly quota overages return 429 with codes `daily_quota_exceeded` / `monthly_quota_exceeded` - `Retry-After` can be hours, so `_lib.sh` caps wait at 60s and errors out beyond that.

## Idempotency

`Idempotency-Key` header is supported **only on**:
- `POST /emails`
- `POST /emails/batch`

Constraints: 1–256 chars, stored 24h.

| Failure case                                                | HTTP | Error code                       |
|-------------------------------------------------------------|------|----------------------------------|
| Same key, different request body                            | 409  | `invalid_idempotent_request`     |
| Same key still in flight                                    | 409  | `concurrent_idempotent_requests` |
| Key not 1–256 chars                                         | 400  | `invalid_idempotency_key`        |

## Pagination

Cursor-based, response shape:

```json
{
  "object": "list",
  "has_more": true,
  "data": [ { "id": "abc", ... }, ... ]
}
```

Params (where supported): `limit` (1–100, default 20), `after=<id>`, `before=<id>`. `after` and `before` are mutually exclusive.

Bash pagination loop (`_lib.sh:resend_paginate`):

```bash
path="/emails?limit=100"
while :; do
  resp=$(resend_api GET "$path")
  printf '%s' "$resp" | jq -c '.data[]?'
  [[ "$(printf '%s' "$resp" | jq -r '.has_more // false')" == "true" ]] || break
  last=$(printf '%s' "$resp" | jq -r '.data[-1].id')
  path=$(printf '%s' "$path" | sed -E 's/(after=)[^&]*/\1'"$last"'/')
  [[ "$path" == *"after="* ]] || path="${path}&after=${last}"
done
```

## Endpoints by resource

### Emails

| Method | Path                                            | Purpose                                  |
|--------|-------------------------------------------------|------------------------------------------|
| POST   | `/emails`                                       | Send a single email                      |
| POST   | `/emails/batch`                                 | Send up to 100 emails (no attachments, no scheduled_at) |
| GET    | `/emails`                                       | List sent emails                         |
| GET    | `/emails/{id}`                                  | Get a single sent email                  |
| PATCH  | `/emails/{id}`                                  | Update `scheduled_at` of a scheduled email |
| DELETE | `/emails/{id}`                                  | Cancel a scheduled email                 |
| GET    | `/emails/{id}/attachments`                      | List attachments on a sent email         |
| GET    | `/emails/{id}/attachments/{attachment_id}`      | Get a single attachment                  |

**Send body (key fields):**
```json
{
  "from": "Acme <hi@acme.com>",
  "to": ["user@example.com"],
  "subject": "Welcome",
  "html": "<p>Hello</p>",
  "text": "Hello",
  "cc": [], "bcc": [], "reply_to": [],
  "scheduled_at": "in 1 hour",
  "headers": { "X-Entity-Ref-ID": "..." },
  "attachments": [{"filename": "f.pdf", "content": "<base64>"}],
  "tags": [{"name": "category", "value": "transactional"}],
  "topic_id": "uuid",
  "template": { "id": "uuid", "variables": {} }
}
```

### Received emails (inbound)

| Method | Path                                                        | Purpose                          |
|--------|-------------------------------------------------------------|----------------------------------|
| GET    | `/received-emails`                                          | List inbound received emails     |
| GET    | `/received-emails/{id}`                                     | Get one received email           |
| GET    | `/received-emails/{id}/attachments`                         | List attachments                 |
| GET    | `/received-emails/{id}/attachments/{attachment_id}`         | Download an attachment           |

### Domains

| Method | Path                       | Purpose                              |
|--------|----------------------------|--------------------------------------|
| POST   | `/domains`                 | Create a domain                      |
| GET    | `/domains`                 | List all domains                     |
| GET    | `/domains/{id}`            | Get a single domain (incl. DNS records) |
| PATCH  | `/domains/{id}`            | Update tracking / TLS settings       |
| DELETE | `/domains/{id}`            | Delete a domain                      |
| POST   | `/domains/{id}/verify`     | Trigger DNS verification             |

Regions on create: `us-east-1`, `eu-west-1`, `sa-east-1`, `ap-northeast-1`. Default `us-east-1`.

### API keys

| Method | Path                  | Purpose                                          |
|--------|-----------------------|--------------------------------------------------|
| POST   | `/api-keys`           | Create a key (token shown once)                  |
| GET    | `/api-keys`           | List keys (no tokens returned)                   |
| DELETE | `/api-keys/{id}`      | Delete a key                                     |

Create body: `{ "name": "label", "permission": "full_access"|"sending_access", "domain_id": "..." }`. `domain_id` only valid with `sending_access`.

### Contacts

| Method | Path                                       | Purpose                                  |
|--------|--------------------------------------------|------------------------------------------|
| POST   | `/contacts`                                | Create a contact                         |
| GET    | `/contacts`                                | List contacts                            |
| GET    | `/contacts/{id}`                           | Get a contact                            |
| PATCH  | `/contacts/{id}`                           | Update                                   |
| DELETE | `/contacts/{id}`                           | Delete                                   |
| POST   | `/contacts/{id}/segments`                  | Add to segment (body `{segment_id}`)     |
| DELETE | `/contacts/{id}/segments/{segment_id}`     | Remove from segment                      |
| GET    | `/contacts/{id}/segments`                  | List segments the contact belongs to     |
| GET    | `/contacts/{id}/topics`                    | List topic subscriptions                 |
| PATCH  | `/contacts/{id}/topics`                    | Update topic subscriptions               |

Custom properties go in `{ properties: { plan: "pro", ... } }`.

### Contact properties (schema)

| Method | Path                            | Purpose                |
|--------|---------------------------------|------------------------|
| POST   | `/contact-properties`           | Create a property      |
| GET    | `/contact-properties`           | List properties        |
| GET    | `/contact-properties/{id}`      | Get a property         |
| PATCH  | `/contact-properties/{id}`      | Update                 |
| DELETE | `/contact-properties/{id}`      | Delete                 |

Body: `{ name, type: "string"|"number"|"boolean"|"date", description }`.

### Segments

| Method | Path                              | Purpose                          |
|--------|-----------------------------------|----------------------------------|
| POST   | `/segments`                       | Create a segment                 |
| GET    | `/segments`                       | List segments                    |
| GET    | `/segments/{id}`                  | Get a segment                    |
| GET    | `/segments/{id}/contacts`         | List contacts in the segment     |
| DELETE | `/segments/{id}`                  | Delete                           |

### Topics

| Method | Path             | Purpose                  |
|--------|------------------|--------------------------|
| POST   | `/topics`        | Create a topic           |
| GET    | `/topics`        | List topics              |
| GET    | `/topics/{id}`   | Get a topic              |
| PATCH  | `/topics/{id}`   | Update                   |
| DELETE | `/topics/{id}`   | Delete                   |

### Broadcasts

| Method | Path                          | Purpose                                       |
|--------|-------------------------------|-----------------------------------------------|
| POST   | `/broadcasts`                 | Create a draft                                |
| GET    | `/broadcasts`                 | List broadcasts                               |
| GET    | `/broadcasts/{id}`            | Get a broadcast                               |
| PATCH  | `/broadcasts/{id}`            | Update a draft                                |
| POST   | `/broadcasts/{id}/send`       | Send (or schedule via `{scheduled_at}` body)  |
| DELETE | `/broadcasts/{id}`            | Delete                                        |

### Templates

| Method | Path                           | Purpose                          |
|--------|--------------------------------|----------------------------------|
| POST   | `/templates`                   | Create                           |
| GET    | `/templates`                   | List                             |
| GET    | `/templates/{id}`              | Get                              |
| PATCH  | `/templates/{id}`              | Update                           |
| DELETE | `/templates/{id}`              | Delete                           |
| POST   | `/templates/{id}/publish`      | Publish a draft                  |
| POST   | `/templates/{id}/duplicate`    | Duplicate (optional `{name}`)    |

### Webhooks

| Method | Path                   | Purpose                                |
|--------|------------------------|----------------------------------------|
| POST   | `/webhooks`            | Create (body: `{endpoint, events[]}`)  |
| GET    | `/webhooks`            | List                                   |
| GET    | `/webhooks/{id}`       | Get                                    |
| PATCH  | `/webhooks/{id}`       | Update                                 |
| DELETE | `/webhooks/{id}`       | Delete                                 |

Common event types: `email.sent`, `email.delivered`, `email.delivery_delayed`, `email.complained`, `email.bounced`, `email.opened`, `email.clicked`, `email.failed`, `contact.created`, `contact.updated`, `contact.deleted`, `domain.created`, `domain.updated`, `domain.deleted`.

### Automations and events

The source skill inferred several raw automation and event-schema paths. They are intentionally excluded from this Codex adaptation. Use the documented native CLI surface instead:

```bash
bunx --bun resend-cli@latest automations <command> ... -q
bunx --bun resend-cli@latest events <command> ... -q
```

Inspect current help before use. Do not translate those commands back into guessed REST routes.

### Logs *(MCP gap)*

| Method | Path             | Purpose                                  |
|--------|------------------|------------------------------------------|
| GET    | `/logs`          | List request logs (cursor pagination)    |
| GET    | `/logs/{id}`     | Get a single log entry                   |

Filter params on `/logs`: `method`, `status_code`, `path` (substring). Retention is plan-gated.

## Error codes

Full table from https://resend.com/docs/api-reference/errors:

| Code                              | HTTP | Meaning                                  |
|-----------------------------------|------|------------------------------------------|
| `validation_error`                | 400  | Field validation failed                  |
| `invalid_idempotency_key`         | 400  | Key not 1–256 chars                      |
| `missing_api_key`                 | 401  | Authorization header absent              |
| `restricted_api_key`              | 401  | Key scoped to sending-only               |
| `invalid_api_key`                 | 403  | Key incorrect/expired                    |
| `not_found`                       | 404  | Endpoint does not exist                  |
| `method_not_allowed`              | 405  | Wrong HTTP method                        |
| `invalid_idempotent_request`      | 409  | Same key, different body                 |
| `concurrent_idempotent_requests`  | 409  | Key still processing                     |
| `invalid_attachment`              | 422  | Attachment missing content/path          |
| `invalid_from_address`            | 422  | From format invalid                      |
| `missing_required_field`          | 422  | Required field absent                    |
| `monthly_quota_exceeded`          | 429  | Monthly email cap hit                    |
| `daily_quota_exceeded`            | 429  | Daily email cap hit                      |
| `rate_limit_exceeded`             | 429  | Req frequency too high                   |
| `security_error`                  | 451  | Security issue detected                  |
| `application_error`               | 500  | Server fault                             |
| `internal_server_error`           | 500  | Server fault                             |
| `1010`                            | 403  | Missing or empty `User-Agent` header     |

Error response shape:

```json
{
  "statusCode": 422,
  "name": "missing_required_field",
  "message": "from field is required"
}
```
