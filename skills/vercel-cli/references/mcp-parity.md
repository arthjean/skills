# Legacy Vercel MCP parity map

Read this file only when translating a workflow that names Vercel MCP tools. MCP tool names and availability can change, so treat the left column as legacy input and verify the current CLI side with `bunx vercel@latest <command> --help`.

Assume:

```bash
VERCEL_SKILL_DIR=/home/arthur/.agents/skills/vercel-cli
export VERCEL_TOKEN=vcp_xxxxxxxxxxxxxxxxxxxxxxxx
```

| Legacy MCP intent or tool | Codex CLI route |
|---|---|
| Search Vercel documentation | Use Context7 with official Vercel documentation, then inspect current CLI help for flags |
| `list_teams` | `bunx vercel@latest teams list` |
| `list_projects` | `bunx vercel@latest projects list [--scope <team>]` |
| `get_project` | `bunx vercel@latest projects inspect <name-or-id>` |
| `list_deployments` | `bunx vercel@latest list [project]` |
| `get_deployment` | `bunx vercel@latest inspect <url-or-id> --format=json` |
| `get_deployment_build_logs` | `bunx vercel@latest inspect <url-or-id> --logs` |
| `get_runtime_logs` | `bunx vercel@latest logs <url-or-id> --json` or `--follow` |
| `check_domain_availability_and_price` | `bunx vercel@latest domains check <domain>` then `domains price <domain>` |
| `buy_domain` | `bunx vercel@latest domains buy <domain>` after explicit approval |
| `get_access_to_vercel_url` | `bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" create <project>` |
| `web_fetch_vercel_url` | `bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" fetch <url> <bypass-secret>` |
| `use_vercel_cli` | Inspect `bunx vercel@latest --help` or command-specific help |
| `deploy_to_vercel` | `bash "$VERCEL_SKILL_DIR/scripts/vercel-deploy.sh" [--prod]` |

## Deployment diagnosis

Replace a multi-tool MCP flow with CLI JSON and logs:

```bash
bunx vercel@latest list <project> --prod
bunx vercel@latest inspect <deployment-id-or-url> --format=json
bunx vercel@latest inspect <deployment-id-or-url> --logs
bunx vercel@latest logs <deployment-id-or-url> --level error --since 1h --json
```

Do not parse the human table from `list` with `head`, `tail`, or fixed columns when an explicit deployment ID is available. If reliable pagination or filtering is required, use `vercel api` or `vercel-api.sh`.

## Protected deployment access

Create a short-lived bypass token only when the user needs access to a protected deployment:

```bash
TOKEN_JSON=$(bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" create <project>)
TOKEN_ID=$(printf '%s' "$TOKEN_JSON" | jq -r '.id')
SECRET=$(printf '%s' "$TOKEN_JSON" | jq -r '.secret')

printf '%s' "$SECRET" \
  | bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" fetch \
      https://example.vercel.app/private -

bash "$VERCEL_SKILL_DIR/scripts/vercel-bypass.sh" rm <project> "$TOKEN_ID"
```

Do not print `SECRET`, append it to a shared URL, or leave the token active after temporary use.

## Additional CLI coverage

Use native CLI families for environment variables, domains, aliases, Edge Config, webhooks, cache, feature flags, redirects, rolling releases, observability, and marketplace integrations. Use [commands.md](commands.md) to route the request.

Use bundled helpers for non-interactive typed environment-variable writes, deterministic Edge Config patch files, project-filtered webhooks, deployment-protection bypass tokens, structured deploy output, or raw REST gaps. See [rest-api.md](rest-api.md) only when a current native command is insufficient.
