# Clerk CLI command reference

This reference was verified against the official `clerk` package version 2.2.0 on 2026-07-16. Always prefer `bunx clerk@latest --mode agent <command> --help` when syntax differs.

Sources:

- [Official Clerk CLI documentation](https://clerk.com/docs/cli)
- [Official Clerk CLI package](https://www.npmjs.com/package/clerk)
- Local package metadata and `--help` output from `clerk@2.2.0`

## Invocation

```bash
bunx clerk@latest --mode agent <command>
```

The package name is `clerk`. The historical `@clerk/dev-cli` package name is not the current invocation.

Global options:

| Option | Purpose |
|---|---|
| `-v, --version` | Print the installed CLI version |
| `--input-json <json|@file|->` | Pass command options as JSON, a file, or stdin |
| `--mode <human|agent>` | Force interaction and output mode |
| `--verbose` | Include debug output |
| `-h, --help` | Show current command help |

Use `--mode agent` for Codex. It avoids interactive selection and returns machine-oriented output. Do not pass a literal secret through `--secret-key`; export `CLERK_SECRET_KEY` instead.

## Current command groups

| Command | Purpose | Important boundary |
|---|---|---|
| `init` | Detect framework, install Clerk SDK, and scaffold auth | Writes project files and dependencies |
| `auth login` | Authenticate the CLI account with OAuth | Opens a browser and stores credentials |
| `auth logout` | Remove stored account credentials | Mutates local credential state |
| `link` / `unlink` | Connect or disconnect the current project | Mutates project context |
| `whoami` | Show account and linked application | Read-only target check |
| `open` | Open a Clerk resource | Browser-only |
| `apps list` | List applications | Read-only |
| `apps create <name>` | Create an application through the Platform API | Account-level mutation |
| `users list` | List and filter users | Read-only |
| `users create` | Create a user | Supports `--dry-run` and `--yes` |
| `users open` | Open a user in the dashboard | Browser-only unless `--print` |
| `impersonate` / `imp` | Create or revoke impersonation | Security-sensitive and token-producing |
| `env pull` | Pull instance keys into a local env file | Secret-bearing file write |
| `config pull` | Read instance configuration | May contain sensitive configuration |
| `config schema` | Inspect supported config keys | Read-only |
| `config patch` | Partially update instance configuration | Supports `--dry-run` |
| `config put` | Replace full instance configuration | Wide-impact mutation |
| `enable` / `disable` | Toggle Clerk features | Instance-wide mutation |
| `api` | Call Backend, Platform, or Frontend API endpoints | Supports discovery and dry-run |
| `doctor` | Diagnose the current integration | `--fix` writes changes |
| `deploy` | Deploy an application to production | Production-wide operation |
| `deploy status` | Inspect production deploy completeness | Read-only |
| `webhooks token` | Create a stable relay token | Secret-producing |
| `webhooks listen` | Stream and forward webhook events | Long-running local process |
| `webhooks verify` | Verify a delivery signature locally | Offline and read-only |
| `completion` | Generate shell completion | Writes only when redirected |
| `update` | Update a global CLI installation | Do not use with ephemeral bunx |

## Target resolution

Use the linked current project:

```bash
bunx clerk@latest --mode agent whoami
```

Or target an application and instance explicitly when the command supports it:

```bash
bunx clerk@latest --mode agent users list \
  --app app_xxx \
  --instance dev \
  --json
```

Instance values can be `dev`, `prod`, or a full instance ID. For mutations, explicit targeting is preferable when more than one target is plausible.

## Users

List users as JSON:

```bash
bunx clerk@latest --mode agent users list \
  --json \
  --limit 100 \
  --offset 0 \
  --order-by=-created_at
```

Current list filters include `--query`, `--email-address`, `--phone-number`, `--username`, `--user-id`, and `--external-id`. The native CLI accepts a maximum `--limit` of 250 even though raw Backend API endpoints can accept different limits.

Preview creation:

```bash
bunx clerk@latest --mode agent users create \
  --email alice@example.com \
  --first-name Alice \
  --dry-run
```

After the exact target and payload are authorized:

```bash
bunx clerk@latest --mode agent users create \
  --email alice@example.com \
  --first-name Alice \
  --yes \
  --json
```

For complex request bodies, use `--file <path>` rather than placing passwords or sensitive metadata in process arguments.

## Backend API proxy

Discover endpoints:

```bash
bunx clerk@latest --mode agent api ls
bunx clerk@latest --mode agent api ls users
```

Read a resource:

```bash
bunx clerk@latest --mode agent api /users/user_xxx
```

Preview a mutation:

```bash
bunx clerk@latest --mode agent api /users/user_xxx \
  -X PATCH \
  -d '{"first_name":"Alice"}' \
  --dry-run
```

Execute only after inspection:

```bash
bunx clerk@latest --mode agent api /users/user_xxx \
  -X PATCH \
  -d '{"first_name":"Alice"}' \
  --yes
```

`api` defaults to GET, or POST when a body is provided. It supports `-X/--method`, `-d/--data`, `--file`, `--include`, `--app`, `--instance`, `--platform`, `--fapi`, `--dry-run`, and `--yes`.

Use `--platform` only for Platform API work explicitly in scope. Use `--fapi` only for public, unauthenticated Frontend API reads. The bundled Bash helpers target the Backend API only.

## Configuration

Inspect schema and current values before changing config:

```bash
bunx clerk@latest --mode agent config schema --keys organization_settings
bunx clerk@latest --mode agent config pull
```

Preview and apply a narrow patch:

```bash
bunx clerk@latest --mode agent config patch \
  --json '{"organization_settings":{"enabled":true}}' \
  --dry-run

bunx clerk@latest --mode agent config patch \
  --json '{"organization_settings":{"enabled":true}}'
```

Use `config put` only for an explicit full replacement. Prefer `patch` for local changes.

## Browser and long-running commands

Do not run these implicitly:

- `auth login`
- `open`
- `users open` without `--print`
- `imp --open`
- interactive production `deploy`
- `webhooks listen`

For a dashboard URL without navigation, use a print option where available:

```bash
bunx clerk@latest --mode agent users open user_xxx --print
```

For any undocumented flag, run the narrowest current help command:

```bash
bunx clerk@latest --mode agent <group> <subcommand> --help
```
