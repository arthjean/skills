# Clerk Backend API - direct `curl` reference

When the CLI doesn't cover a resource and the resource-specific helper isn't enough, hit the API directly. This file documents the boilerplate, every resource category, and the gotchas.

## Boilerplate

```bash
curl -fsS \
  -H "Authorization: Bearer $CLERK_SECRET_KEY" \
  -H "Clerk-API-Version: 2026-05-12" \
  -H "Accept: application/json" \
  "https://api.clerk.com/v1/<path>"
```

For POST/PATCH:

```bash
curl -fsS \
  -X POST \
  -H "Authorization: Bearer $CLERK_SECRET_KEY" \
  -H "Clerk-API-Version: 2026-05-12" \
  -H "Content-Type: application/json" \
  -d '{"name":"Acme"}' \
  "https://api.clerk.com/v1/organizations"
```

The `clerk_api()` function in `scripts/_lib.sh` wraps this pattern with one bounded 429 retry. Prefer `bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" METHOD PATH [body]` for scripts.

## Base URL & version

| | Value |
|---|---|
| Base URL | `https://api.clerk.com/v1` |
| Auth | `Authorization: Bearer $CLERK_SECRET_KEY` (sk_test_/sk_live_) |
| Version header | `Clerk-API-Version: 2026-05-12` |
| Pinning rule | Use exactly one version mechanism: the header or `__clerk_api_version`, never both |

The helpers pin the current stable contract verified on 2026-07-16. API version `2026-05-12` moves user and organization metadata updates to dedicated endpoints. Source: [Clerk versioning overview](https://clerk.com/docs/guides/development/upgrading/versioning).

## Pagination

Classic `limit` + `offset`. No cursors.

| Param | Default | Max |
|---|---|---|
| `limit` | 10 | 500 |
| `offset` | 0 | (no documented cap) |
| `order_by` | varies | `-created_at`, `created_at`, `-updated_at`, `updated_at` |

Iterate manually for large tables:

```bash
total=$(bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET /users/count | jq -r '.total_count')
offset=0
while [[ $offset -lt $total ]]; do
  bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET "/users?limit=500&offset=$offset" \
    | jq -c '.[]' >> all-users.ndjson
  offset=$((offset + 500))
done
```

## Rate limits

Source: [clerk.com/docs/guides/how-clerk-works/system-limits](https://clerk.com/docs/guides/how-clerk-works/system-limits).

| Endpoint context | Limit |
|---|---|
| Production instances | 1,000 req / 10s |
| Development instances | 100 req / 10s |
| `GET /jwks` | No rate limit |
| `POST /invitations` | 100 / hr |
| `POST /invitations/bulk` | 25 / hr |
| `POST /organizations/{id}/invitations` | 250 / hr |
| `POST /organizations/{id}/invitations/bulk` | 50 / hr |
| User metadata update endpoints | 10 / 10s per user |
| Organization metadata update endpoints | 10 / 10s per organization |

On 429, the response includes `Retry-After: <seconds>`. `scripts/_lib.sh::clerk_api` retries once and honors it. Pace high-volume development-instance work below the 10 requests per second aggregate limit:

```bash
for u in $(jq -r '.[].id' users.json); do
  bash "$CLERK_SKILL_DIR/scripts/clerk-users.sh" metadata "$u" public '{"migrated":true}'
  sleep 0.12
done
```

## Resource catalog

Every `<path>` below is appended to `https://api.clerk.com/v1`. `{id}` placeholders are typed (e.g. `user_xxx`, `org_xxx`, `sess_xxx`).

### Users

| Method | Path | Purpose |
|---|---|---|
| GET | `/users` | List users (paginated) |
| GET | `/users/count` | Count |
| GET | `/users/{id}` | Get single user |
| POST | `/users` | Create user |
| PATCH | `/users/{id}` | Update user |
| PATCH | `/users/{id}/metadata` | Deep-merge selected metadata fields |
| PUT | `/users/{id}/metadata` | Replace selected metadata fields |
| DELETE | `/users/{id}` | Delete user (irreversible) |
| POST | `/users/{id}/ban` | Ban (reversible) |
| POST | `/users/{id}/unban` | Unban |
| POST | `/users/{id}/lock` | Lock (auto unlocks after configured TTL) |
| POST | `/users/{id}/unlock` | Force-unlock |
| GET | `/users/{id}/organization_memberships` | List orgs the user belongs to |
| POST | `/users/{id}/profile_image` | Set profile image (multipart) |
| DELETE | `/users/{id}/profile_image` | Remove profile image |
| GET | `/users/{id}/oauth_access_tokens/{provider}` | Read OAuth access token (e.g. for `oauth_google`) |
| POST | `/users/{id}/verify_password` | Verify a user's password |
| POST | `/users/{id}/verify_totp` | Verify a TOTP code |

### Organizations + memberships + invitations + domains

| Method | Path | Purpose |
|---|---|---|
| GET | `/organizations` | List |
| POST | `/organizations` | Create (`created_by` required) |
| GET | `/organizations/{id}` | Get |
| PATCH | `/organizations/{id}` | Update |
| PATCH | `/organizations/{id}/metadata` | Deep-merge selected metadata fields |
| PUT | `/organizations/{id}/metadata` | Replace selected metadata fields |
| DELETE | `/organizations/{id}` | Delete |
| GET | `/organizations/{id}/memberships` | List members |
| POST | `/organizations/{id}/memberships` | Add member (`user_id`, `role`) |
| PATCH | `/organizations/{id}/memberships/{user_id}` | Update role |
| DELETE | `/organizations/{id}/memberships/{user_id}` | Remove member |
| GET | `/organizations/{id}/invitations` | List org invitations |
| POST | `/organizations/{id}/invitations` | Create org invitation |
| POST | `/organizations/{id}/invitations/bulk` | Bulk org invitations (rate-limited 50/hr) |
| GET | `/organizations/{id}/invitations/{inv_id}` | Get |
| POST | `/organizations/{id}/invitations/{inv_id}/revoke` | Revoke |
| GET | `/organizations/{id}/domains` | List org domains |
| POST | `/organizations/{id}/domains` | Add (`name`, `enrollment_mode`) |
| DELETE | `/organizations/{id}/domains/{dom_id}` | Remove |

### Sessions

| Method | Path | Purpose |
|---|---|---|
| GET | `/sessions` | List (filter `?user_id=...`, `?status=active`) |
| GET | `/sessions/{id}` | Get |
| POST | `/sessions/{id}/revoke` | Force-logout this session |
| POST | `/sessions/{id}/verify` | Verify a session token |
| POST | `/sessions/{id}/tokens/{template_name}` | Mint a JWT against a template |

### Invitations (app-level)

| Method | Path | Purpose |
|---|---|---|
| GET | `/invitations` | List |
| POST | `/invitations` | Create (rate limit 100/hr) |
| POST | `/invitations/bulk` | Bulk (rate limit 25/hr) |
| POST | `/invitations/{id}/revoke` | Revoke |

### Email addresses & phone numbers

| Method | Path | Purpose |
|---|---|---|
| POST | `/email_addresses` | Add to a user |
| GET | `/email_addresses/{id}` | Get |
| PATCH | `/email_addresses/{id}` | Update (e.g. mark verified) |
| DELETE | `/email_addresses/{id}` | Remove |
| POST | `/phone_numbers` | Add |
| GET | `/phone_numbers/{id}` | Get |
| PATCH | `/phone_numbers/{id}` | Update |
| DELETE | `/phone_numbers/{id}` | Remove |

### Allowlist / blocklist

| Method | Path | Purpose |
|---|---|---|
| GET | `/allowlist_identifiers` | List |
| POST | `/allowlist_identifiers` | Add (`identifier`, `notify`) |
| DELETE | `/allowlist_identifiers/{id}` | Remove |
| GET | `/blocklist_identifiers` | List |
| POST | `/blocklist_identifiers` | Add |
| DELETE | `/blocklist_identifiers/{id}` | Remove |

### JWT templates

| Method | Path | Purpose |
|---|---|---|
| GET | `/jwt_templates` | List |
| POST | `/jwt_templates` | Create (`name`, `claims`, `lifetime`) |
| GET | `/jwt_templates/{id}` | Get |
| PATCH | `/jwt_templates/{id}` | Update |
| DELETE | `/jwt_templates/{id}` | Delete |

### Instance settings

| Method | Path | Purpose |
|---|---|---|
| GET | `/instance` | Read full instance config |
| PATCH | `/instance` | Patch (e.g. `home_origin`) |
| PATCH | `/instance/restrictions` | Toggle allowlist/blocklist |
| PATCH | `/instance/organization_settings` | `max_allowed_memberships`, `creator_role`, etc. |
| GET | `/jwks` | Public JWKS for token verification (no rate limit) |

### Domains & redirect URLs (instance-level)

| Method | Path | Purpose |
|---|---|---|
| GET | `/domains` | List |
| POST | `/domains` | Add (`name`, `is_satellite`) |
| DELETE | `/domains/{id}` | Remove |
| GET | `/redirect_urls` | List |
| POST | `/redirect_urls` | Add |
| GET | `/redirect_urls/{id}` | Get |
| DELETE | `/redirect_urls/{id}` | Remove |

### OAuth applications + SAML

| Method | Path | Purpose |
|---|---|---|
| GET | `/oauth_applications` | List Clerk-acting-as-OAuth-provider apps |
| POST | `/oauth_applications` | Create (`name`, `redirect_uris`, `scopes`, `public`) |
| GET | `/oauth_applications/{id}` | Get |
| PATCH | `/oauth_applications/{id}` | Update |
| POST | `/oauth_applications/{id}/rotate_secret` | Rotate client secret |
| DELETE | `/oauth_applications/{id}` | Delete |
| GET | `/saml_connections` | List |
| POST | `/saml_connections` | Create |
| GET | `/saml_connections/{id}` | Get |
| PATCH | `/saml_connections/{id}` | Update |
| DELETE | `/saml_connections/{id}` | Delete |

### Sign-in / actor / testing tokens

| Method | Path | Purpose |
|---|---|---|
| POST | `/sign_in_tokens` | Magic-link style one-time sign-in token |
| POST | `/sign_in_tokens/{id}/revoke` | Revoke before use |
| POST | `/actor_tokens` | "Sign in as" - admin impersonation |
| POST | `/actor_tokens/{id}/revoke` | Revoke |
| POST | `/testing_tokens` | E2E test bypass tokens (test instance only) |
| POST | `/proxy_checks` | Verify a custom auth proxy is reachable |

### Sign-ups & sign-ins (introspection)

| Method | Path | Purpose |
|---|---|---|
| GET | `/sign_ups/{id}` | Inspect a sign-up attempt |
| PATCH | `/sign_ups/{id}` | Update (e.g. abandon, attribute additional fields) |
| GET | `/sign_ins/{id}` | Inspect a sign-in attempt |

### Clients

| Method | Path | Purpose |
|---|---|---|
| GET | `/clients` | List active client (browser) sessions |
| GET | `/clients/{id}` | Get |
| GET | `/clients/verify` | Verify a client JWT (server-side helper) |

### M2M API keys

| Method | Path | Purpose |
|---|---|---|
| POST | `/api_keys` | Mint a machine API key |
| GET | `/api_keys/{id}` | Get |
| DELETE | `/api_keys/{id}` | Revoke |

## Backend API boundary

The Backend API manages resources inside one Clerk instance. Account and workspace operations use the Platform API or the dashboard instead:

- Use `clerk apps list` and `clerk apps create` for application management.
- Use `clerk config schema`, `config pull`, and `config patch` for instance configuration, including supported social connection settings.
- Use `clerk api --platform` only when the user explicitly requests a Platform API operation.
- Analytics, usage metrics, application and email logs, workspace membership, subscription billing, Account Portal branding, and Clerk Protect settings remain dashboard surfaces according to the current CLI documentation.

Do not infer that a missing Backend API path is impossible across all Clerk surfaces. Check current `clerk --help`, `clerk api ls`, and `clerk api --platform ls`.

## Useful jq recipes

```bash
# Just IDs and primary emails
bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET /users \
  | jq '.[] | {id, email: .email_addresses[0].email_address}'

# Count active sessions per user
bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET "/sessions?status=active&limit=500" \
  | jq 'group_by(.user_id) | map({user_id: .[0].user_id, n: length}) | sort_by(.n) | reverse'

# All orgs the user user_xxx is in, with role
bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET /users/user_xxx/organization_memberships \
  | jq '.[] | {org: .organization.name, role}'

# Users created in the last 24h
since=$(date -d '24 hours ago' +%s)000   # ms epoch
bash "$CLERK_SKILL_DIR/scripts/clerk-api.sh" GET "/users?limit=500&order_by=-created_at" \
  | jq --argjson since "$since" '[.[] | select(.created_at >= $since)]'
```

## OpenAPI spec

Clerk publishes the current Backend API reference at [clerk.com/docs/reference/backend-api](https://clerk.com/docs/reference/backend-api). The versioned OpenAPI source used by these helpers is [bapi/2026-05-12.yml](https://github.com/clerk/openapi-specs/blob/main/bapi/2026-05-12.yml). Use `bunx clerk@latest --mode agent api ls` to inspect the current CLI catalog before adding a raw path.
