# Agent Skills

Public collection of Agent Skills used by Arthur across Codex, Claude Code, and other compatible coding agents.

## Install

Install from GitHub with the [`skills` CLI](https://github.com/vercel-labs/skills):

```bash
bunx skills add arthjean/skills
```

Inspect the catalog without installing anything:

```bash
bunx skills add arthjean/skills --list
```

Install one or more specific skills:

```bash
bunx skills add arthjean/skills --skill meta-code --skill write-prd
```

Install globally instead of in the current project:

```bash
bunx skills add arthjean/skills --global
```

## Native installer

The `arthur-skills` binary embeds the complete runtime catalog and requires no JavaScript runtime after compilation. Rust 1.95.0 is pinned at the repository root.

```bash
# Pin acquisition to a release, then verify the downloaded checksum file before execution.
ARTHUR_SKILLS_VERSION=v0.1.0
curl --proto '=https' --tlsv1.2 -LsSf \
  "https://github.com/arthjean/skills/releases/download/${ARTHUR_SKILLS_VERSION}/arthur-skills-installer.sh" \
  -o arthur-skills-installer.sh
curl --proto '=https' --tlsv1.2 -LsSf \
  "https://github.com/arthjean/skills/releases/download/${ARTHUR_SKILLS_VERSION}/sha256.sum" \
  -o sha256.sum
grep ' arthur-skills-installer.sh$' sha256.sum > arthur-skills-installer.sh.sha256
sha256sum --check arthur-skills-installer.sh.sha256
sh arthur-skills-installer.sh
```

Release `v0.1.0` is the documented contract target; use a published tag and its `sha256.sum` manifest rather than an unpinned main-branch installer. For local development:

```bash
cargo run -p arthur-skills -- --help
cargo build --release -p arthur-skills
```

On Windows, use the signed release inputs from PowerShell:

```powershell
$Version = "v0.1.0"
$Base = "https://github.com/arthjean/skills/releases/download/$Version"
Invoke-WebRequest "$Base/arthur-skills-installer.ps1" -OutFile arthur-skills-installer.ps1
Invoke-WebRequest "$Base/sha256.sum" -OutFile sha256.sum
$Checksum = Get-Content sha256.sum | Where-Object { $_ -match ' arthur-skills-installer\.ps1$' }
if (@($Checksum).Count -ne 1) { throw "Installer checksum entry missing" }
$Expected = $Checksum.Split()[0].ToLowerInvariant()
$Observed = (Get-FileHash arthur-skills-installer.ps1 -Algorithm SHA256).Hash.ToLowerInvariant()
if ($Observed -ne $Expected) { throw "Installer checksum mismatch" }
& .\arthur-skills-installer.ps1
```

The command surface is `plan`, `install`, `status`, `doctor`, `update`, `uninstall`, `adopt`, and `recover`. Start by reviewing the immutable plan, then apply the same provider selection:

```bash
arthur-skills plan --provider claude --provider codex
arthur-skills install --provider claude --provider codex
arthur-skills doctor
```

Interactive terminals use Ratatui in an inline viewport. `--plain`, `ARTHUR_SKILLS_PLAIN=1`, or `TERM=dumb` selects the keyboard-driven line renderer. Redirected streams and `CI=true` never prompt; pass explicit `--provider` values and `--yes` for mutations. `--json` emits one schema-v1 envelope and never initializes Ratatui. `NO_COLOR` disables color without changing the interaction mode.

The installer writes canonical skills under `$HOME/.agents/skills`, Claude activations and agents under `$HOME/.claude/{skills,agents}`, Codex agents under `${CODEX_HOME:-$HOME/.codex}/agents`, and its versioned receipt under `$HOME/.agents/.arthur-workflow/receipt.json`. On Windows, `$HOME` falls back to `%USERPROFILE%`, and Claude activations are managed copies because creating symlinks can require Developer Mode or elevated privileges. Unix uses relative Claude symlinks. The installer owns only entries recorded in its receipt. Matching foreign entries require explicit `adopt`; conflicts and drift are never overwritten or removed. `--dry-run` scans and serializes the same plan without creating directories, locks, staging, timestamps, or a receipt.

The JSON v1 status set is closed: `success`, `noop`, `blocked`, `failed`, or `recovery_required`. Every diagnostic contains `code`, `severity`, `message`, mutually exclusive `path_utf8` or `path_bytes_hex`, and `remediation`. Exit codes are stable: `0` success or no-op, `2` usage or missing non-interactive decisions, `3` conflict, `4` invalid environment, `5` transaction or integrity failure, `130` SIGINT, and `143` SIGTERM.

## Repository structure

- [`skills/`](skills/) contains every installable skill. Each skill has its own `SKILL.md` and optional references, scripts, or assets.
- [`agents/claude/`](agents/claude/) mirrors the agents optimized for Claude Code.
- [`agents/codex/`](agents/codex/) mirrors the agents optimized for Codex, including their evaluation suite.
- [`crates/arthur-skills/`](crates/arthur-skills/) contains the native CLI and build-time catalog validator.
- [`shared/claude/skills/_shared/`](shared/claude/skills/_shared/) contains packaged Claude workflow support documents.

The `skills` CLI does not install these agent files.

The CLI discovers skills recursively, so the repository can be installed directly through the GitHub shorthand above.

## License

Original material authored by Arthur is available under the MIT License. Bundled upstream skills and components retain their respective licenses and copyrights. See [THIRD_PARTY.md](THIRD_PARTY.md).
