# Vercel CLI command map

This map was checked against Vercel CLI 56.2.0. Treat it as a routing reference, then inspect `bunx vercel@latest <command> --help` before using an unfamiliar or destructive flag.

Assume `VERCEL_TOKEN` is exported. Keep it in the environment instead of passing `--token`, which can expose it through process arguments or logs.

## Global options

| Option | Purpose |
|---|---|
| `--cwd <dir>` | Set the working directory for one command |
| `--scope <team>` or `-S` | Select a team scope |
| `--token <token>` or `-t` | Override environment authentication; avoid in Codex unless unavoidable |
| `--non-interactive` | Disable interactive prompts; Codex is normally detected automatically |
| `--no-color` | Remove ANSI formatting |
| `--debug` | Enable verbose diagnostics; scrub output before reporting it |
| `--yes` or `-y` | Accept prompts; use only after confirming the exact mutation target |

Do not run `login` or `open` without explicit browser permission.

## Authentication and scope

```bash
bunx vercel@latest whoami
bunx vercel@latest teams list
bunx vercel@latest teams members --scope <team>
```

Prefer `--scope` for one-off commands. Use `switch` only when the user wants to change persisted CLI scope.

## Project context

```bash
bunx vercel@latest projects list --scope <team>
bunx vercel@latest projects inspect <name-or-id> --scope <team>
bunx vercel@latest projects add <name> --scope <team>
bunx vercel@latest projects remove <name> --scope <team>
```

Link the current repository only when persistent local context is useful:

```bash
bunx vercel@latest link --project <name-or-id> --scope <team> --yes
```

This writes `.vercel/project.json`. Keep the current working directory in the repository.

## Deployments

```bash
# Inspect inputs without deploying
bunx vercel@latest deploy --dry --format=json

# Preview deployment
bunx vercel@latest deploy --yes --format=json

# Production deployment
bunx vercel@latest deploy --prod --yes --format=json

# Custom environment
bunx vercel@latest deploy --target <environment> --yes --format=json

# List and inspect
bunx vercel@latest list [project]
bunx vercel@latest list [project] --prod
bunx vercel@latest inspect <url-or-id> --format=json
bunx vercel@latest inspect <url-or-id> --logs
bunx vercel@latest inspect <url-or-id> --wait --timeout 90s
```

Production routing operations:

```bash
bunx vercel@latest promote <url-or-id>
bunx vercel@latest promote status [project]
bunx vercel@latest rollback <url-or-id>
bunx vercel@latest rollback status [project]
bunx vercel@latest redeploy <url-or-id> [--target <environment>]
```

Removal has two distinct blast radii:

```bash
# Remove one deployment
bunx vercel@latest remove <deployment-id> --safe --yes

# Remove every deployment for a project
bunx vercel@latest remove <project-name> --yes

# Delete the project itself
bunx vercel@latest projects remove <project-name>
```

Prefer a `dpl_...` deployment ID for single-deployment removal.

## Logs and observability

Use `inspect --logs` for build output and `logs` for request or runtime logs:

```bash
bunx vercel@latest inspect <url-or-id> --logs
bunx vercel@latest logs <url-or-id> --json
bunx vercel@latest logs <url-or-id> --follow
bunx vercel@latest logs --project <name-or-id> --level error --since 1h --json
bunx vercel@latest logs --query 'status:500 error' --json
```

Current log filters include deployment, project, environment, branch, source, level, status code, request ID, time range, free-form query, and result limit.

Additional observability families:

```text
activity, agent-runs, alerts, metrics, traces, usage
```

Inspect their help before use because filters evolve quickly.

## Environment variables

```bash
bunx vercel@latest env list [environment] [git-branch]
bunx vercel@latest env add <name> [environment] [git-branch]
bunx vercel@latest env update <name> [environment]
bunx vercel@latest env remove <name> [environment]
bunx vercel@latest env pull [filename] --environment=<environment>
bunx vercel@latest env run -- <command>
```

Native add and update flows may request values interactively. Use `vercel-env.sh` for non-interactive typed or multi-target writes. Never echo a value or pass a secret in a command that will be logged.

## Domains, DNS, certificates, and aliases

```bash
bunx vercel@latest domains list
bunx vercel@latest domains inspect <domain>
bunx vercel@latest domains check <domain>
bunx vercel@latest domains price <domain>
bunx vercel@latest domains verify <domain>
bunx vercel@latest domains add <domain> [project]
bunx vercel@latest domains buy <domain>
bunx vercel@latest domains move <domain> <destination-team>
bunx vercel@latest domains transfer-in <domain>
bunx vercel@latest domains remove <domain>

bunx vercel@latest dns --help
bunx vercel@latest certs --help

bunx vercel@latest alias list
bunx vercel@latest alias set <deployment-id-or-url> <alias>
bunx vercel@latest alias remove <alias>
```

Treat domain purchase, transfer, move, removal, DNS mutation, certificate mutation, and alias removal as externally visible changes.

## Edge Config

The native CLI currently covers store metadata, item patches, tokens, and backups:

```bash
bunx vercel@latest edge-config list
bunx vercel@latest edge-config get <id-or-slug>
bunx vercel@latest edge-config add <slug>
bunx vercel@latest edge-config items <id-or-slug>
bunx vercel@latest edge-config update <id-or-slug> --patch '<json>'
bunx vercel@latest edge-config tokens <id-or-slug>
bunx vercel@latest edge-config backups <id-or-slug>
bunx vercel@latest edge-config remove <id-or-slug>
```

Use `vercel-edge-config.sh` when patch operations already exist in a JSON file or deterministic REST output matters.

## Webhooks

```bash
bunx vercel@latest webhooks list
bunx vercel@latest webhooks get <id>
bunx vercel@latest webhooks create <url> --help
bunx vercel@latest webhooks remove <id>
```

The command family is beta. Inspect `create --help` for the current event and project filters. Use `vercel-webhooks.sh` for explicit project ID lists.

## Authenticated REST requests

Use the native beta API client when its OpenAPI catalog contains the endpoint:

```bash
bunx vercel@latest api list
bunx vercel@latest api /v2/user
bunx vercel@latest api /v9/projects --scope <team> --paginate
bunx vercel@latest api /v10/projects --method POST --field name=my-project
bunx vercel@latest api /v10/projects --method POST --input config.json
bunx vercel@latest api /v13/deployments/dpl_xxx --method DELETE
bunx vercel@latest api /v10/projects --method POST --input config.json --generate=curl
```

DELETE requests may require confirmation. Do not use `--dangerously-skip-permissions` unless the user explicitly authorized the exact target.

Use `vercel-api.sh` when raw JSON input or an endpoint outside the native catalog is required.

## Other current command families

The top-level CLI also exposes these families:

```text
ai-gateway, alerts, blob, build, buy, cache, connect, contract, cron,
curl, deploy-hooks, firewall, flags, git, httpstat, integration,
integration-resource, metrics, microfrontends, oauth-apps, redirects,
rolling-release, routes, sandbox, target, telemetry, tokens, traces,
usage, vcr
```

Load only the relevant help page. Do not expand this skill's context with unrelated command families.

## Documentation lookup

For current command syntax, prefer local CLI help. For semantic or product documentation, use Context7 against official Vercel docs:

```bash
bunx ctx7@latest library vercel "<full question>"
bunx ctx7@latest docs /websites/vercel "<specific Vercel question>"
```

If Context7 returns a different best-match library ID, use that result instead of hard-coding `/websites/vercel`.
