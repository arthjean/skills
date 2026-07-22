# Bundled Resend Bash helper index

Use this reference only when the official CLI does not cover the required workflow or a deterministic REST pipeline is useful. Prefer the native commands in [cli-parity.md](cli-parity.md).

Every script path below means:

```bash
bash "$RESEND_SKILL_DIR/scripts/<script>.sh" ...
```

Keep the working directory in the user's project. Do not invoke `scripts/...` relative to that directory. For full helper usage, run the script with no arguments.

## Preflight

| Script                        | Purpose                                                                              |
|-------------------------------|--------------------------------------------------------------------------------------|
| `scripts/resend-ensure.sh`    | Verify bun/jq/curl + RESEND_API_KEY + live `GET /domains` + permission-tier probe    |

## Raw REST

| Script                  | Usage                                                                              |
|-------------------------|------------------------------------------------------------------------------------|
| `scripts/resend-api.sh` | `METHOD PATH [json_body]` - generic curl with auto auth + UA + 429 retry            |

## Emails

| Subcommand     | Args                                                                                          |
|----------------|-----------------------------------------------------------------------------------------------|
| `send`         | `--from --to --subject --html|--text [--cc --bcc --reply-to --scheduled-at --idempotency-key --tag k=v --attach path --header K=V --topic id]` |
| `batch`        | `<@file.json|json-array>` (≤100, no attachments, no scheduled_at)                              |
| `ls`           | `[--limit N]`                                                                                 |
| `get`          | `<email_id>`                                                                                  |
| `cancel`       | `<email_id>` (scheduled emails)                                                               |
| `reschedule`   | `<email_id> <new_scheduled_at>`                                                               |
| `attachments`  | `<email_id>` *(MCP gap)*                                                                      |
| `attachment`   | `<email_id> <attachment_id>` *(MCP gap)*                                                      |

## Received emails (inbound)

| Subcommand     | Args                                                                |
|----------------|---------------------------------------------------------------------|
| `ls`           | `[--limit N]`                                                       |
| `get`          | `<received_email_id>`                                               |
| `attachments`  | `<received_email_id>`                                               |
| `attachment`   | `<received_email_id> <attachment_id> [--out path]`                  |

## Domains

| Subcommand | Args                                                                                  |
|------------|---------------------------------------------------------------------------------------|
| `create`   | `--name <domain> [--region us-east-1|eu-west-1|sa-east-1|ap-northeast-1] [--click-tracking on|off] [--open-tracking on|off]` |
| `ls`       |                                                                                       |
| `get`      | `<domain_id>`                                                                         |
| `update`   | `<domain_id> [--click-tracking ...] [--open-tracking ...] [--tls ...]`                |
| `verify`   | `<domain_id>`                                                                         |
| `rm`       | `<domain_id>`                                                                         |
| `dns`      | `<domain_id>` - prints just the DNS records to add                                    |

## API keys

| Subcommand | Args                                                                          |
|------------|-------------------------------------------------------------------------------|
| `create`   | `--name <label> --permission full_access|sending_access [--domain <dom_id>] --out <secure-json-path>` |
| `ls`       |                                                                               |
| `rm`       | `<key_id>`                                                                    |

## Contacts

| Subcommand     | Args                                                                                  |
|----------------|---------------------------------------------------------------------------------------|
| `create`       | `--email <a@b.com> [--first --last --unsubscribed --prop k=v ...]`                    |
| `ls`           | `[--limit N]`                                                                         |
| `get`          | `<contact_id>`                                                                        |
| `update`       | `<contact_id> [--email --first --last --unsubscribed true|false --prop k=v ...]`      |
| `rm`           | `<contact_id>`                                                                        |
| `segments`     | `<contact_id>`                                                                        |
| `add-segment`  | `<contact_id> <segment_id>`                                                            |
| `rm-segment`   | `<contact_id> <segment_id>`                                                            |
| `topics`       | `<contact_id>`                                                                        |
| `set-topics`   | `<contact_id> <@file.json|json-body>`                                                  |

## Contact properties

| Subcommand | Args                                                                          |
|------------|-------------------------------------------------------------------------------|
| `create`   | `--name <key> --type string|number|boolean|date [--description ...]`          |
| `ls`       |                                                                               |
| `get`      | `<property_id>`                                                               |
| `update`   | `<property_id> [--name --description]`                                        |
| `rm`       | `<property_id>`                                                               |

## Segments

| Subcommand  | Args                                                          |
|-------------|---------------------------------------------------------------|
| `create`    | `--name [--description --filter @file.json|json]`              |
| `ls`        |                                                               |
| `get`       | `<segment_id>`                                                |
| `contacts`  | `<segment_id> [--limit N]`                                    |
| `rm`        | `<segment_id>`                                                |

## Topics

| Subcommand | Args                                                          |
|------------|---------------------------------------------------------------|
| `create`   | `--name [--description --default-subscribed]`                 |
| `ls`       |                                                               |
| `get`      | `<topic_id>`                                                  |
| `update`   | `<topic_id> [--name --description]`                           |
| `rm`       | `<topic_id>`                                                  |

## Broadcasts

| Subcommand | Args                                                                                                      |
|------------|-----------------------------------------------------------------------------------------------------------|
| `create`   | `--from --subject --html @body.html|--text @body.txt [--name --reply-to --segment <id> --template <id>]`  |
| `ls`       |                                                                                                           |
| `get`      | `<broadcast_id>`                                                                                          |
| `update`   | `<broadcast_id> [--subject --html --text --name --reply-to --from]`                                       |
| `send`     | `<broadcast_id> [--scheduled-at <when>]`                                                                  |
| `rm`       | `<broadcast_id>`                                                                                          |

## Templates

| Subcommand    | Args                                                                                                 |
|---------------|------------------------------------------------------------------------------------------------------|
| `create`      | `--name --subject --html @body.html|--text @body.txt [--from --reply-to --description]`              |
| `ls`          |                                                                                                      |
| `get`         | `<template_id>`                                                                                      |
| `update`      | `<template_id> [--name --subject --html --text --from --reply-to --description]`                     |
| `publish`     | `<template_id>`                                                                                      |
| `duplicate`   | `<template_id> [--name <new_name>]`                                                                  |
| `rm`          | `<template_id>`                                                                                      |

## Webhooks

| Subcommand | Args                                                                                                       |
|------------|------------------------------------------------------------------------------------------------------------|
| `create`   | `--url <https://...> --events evt,evt,... [--name --enabled true|false] --out <secure-json-path>`           |
| `ls`       |                                                                                                            |
| `get`      | `<webhook_id>`                                                                                             |
| `update`   | `<webhook_id> [--url --events --name --enabled]`                                                            |
| `rm`       | `<webhook_id>`                                                                                             |
| `listen`   | `[--port N ...]` - delegates to `bunx --bun resend-cli@latest webhooks listen`                             |

## Automations

`resend-automations.sh` delegates arguments unchanged to the current official CLI. Use native CLI flags:

| Subcommand | Args                                                   |
|------------|--------------------------------------------------------|
| `create`   | `--name <name> --file <workflow.json>`                 |
| `ls`       | `[pagination flags]`                                   |
| `get`      | `<automation_id>`                                      |
| `update`   | `<automation_id> --status enabled|disabled`            |
| `stop`     | `<automation_id>`                                      |
| `runs`     | `<automation_id>` or current nested run commands       |
| `rm`       | `<automation_id> --yes`                                |

## Events

`resend-events.sh` delegates arguments unchanged to the current official CLI. Use native CLI flags:

| Subcommand | Args                                                                                  |
|------------|---------------------------------------------------------------------------------------|
| `send`     | `--event <name> (--contact-id <id> | --email <addr>) [--payload <json>]`               |
| `create`   | `--name <event_name> [--schema <json>]`                                               |
| `ls`       |                                                                                       |
| `get`      | `<event_id>`                                                                          |
| `update`   | `<event_id> --schema <json|null>`                                                     |
| `rm`       | `<event_id> --yes`                                                                    |

## Logs *(MCP gap)*

| Subcommand | Args                                                                                  |
|------------|---------------------------------------------------------------------------------------|
| `ls`       | `[--limit N --method GET|POST|... --status <code> --path <substring>]`                |
| `get`      | `<log_id>`                                                                            |

## Cheat sheet - common one-liners

```bash
# Hello world (auto-loads RESEND_API_KEY from .env.local)
bash "$RESEND_SKILL_DIR/scripts/resend-emails.sh" send --from 'Acme <hi@acme.com>' --to me@dev.com --subject Hi --html '<p>Hi</p>'

# Idempotent transactional send
bash "$RESEND_SKILL_DIR/scripts/resend-emails.sh" send --to user@x.com --subject Welcome --html @welcome.html --idempotency-key signup-42

# Broadcast to a segment
bash "$RESEND_SKILL_DIR/scripts/resend-segments.sh" create --name VIPs --filter @vips.json | jq -r .id
bash "$RESEND_SKILL_DIR/scripts/resend-broadcasts.sh" create --from 'Acme <hi@acme.com>' --subject "Launch" --html @launch.html --segment seg_abc
bash "$RESEND_SKILL_DIR/scripts/resend-broadcasts.sh" send <broadcast_id>

# Add and verify a domain
bash "$RESEND_SKILL_DIR/scripts/resend-domains.sh" create --name acme.com --region eu-west-1
bash "$RESEND_SKILL_DIR/scripts/resend-domains.sh" dns <domain_id>
bash "$RESEND_SKILL_DIR/scripts/resend-domains.sh" verify <domain_id>

# Trigger an automation
bunx --bun resend-cli@latest events send --event user.created --email a@b.com --payload '{"plan":"pro"}' -q

# Debug a 422
bash "$RESEND_SKILL_DIR/scripts/resend-logs.sh" ls --status 422 --limit 20

# All emails sent in the last few pages
bash "$RESEND_SKILL_DIR/scripts/resend-emails.sh" ls --limit 100 | jq -c 'select(.created_at >= "2026-05-01")'
```
