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

## Repository structure

- [`skills/`](skills/) contains every installable skill. Each skill has its own `SKILL.md` and optional references, scripts, or assets.
- [`agents/claude/`](agents/claude/) mirrors the agents optimized for Claude Code.
- [`agents/codex/`](agents/codex/) mirrors the agents optimized for Codex, including their evaluation suite.

The `skills` CLI does not install these agent files.

The CLI discovers skills recursively, so the repository can be installed directly through the GitHub shorthand above.

## License

Original material authored by Arthur is available under the MIT License. Bundled upstream skills and components retain their respective licenses and copyrights. See [THIRD_PARTY.md](THIRD_PARTY.md).
