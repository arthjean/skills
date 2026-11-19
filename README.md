# Agent Skills

Public collection of Agent Skills and agent definitions used by Arthur across Codex, Claude Code, and other compatible coding agents.

The collection is being rebuilt: skills will live under `skills/` and agents under `agents/claude/` and `agents/codex/`.

## Install

Install from GitHub with the [`skills` CLI](https://github.com/vercel-labs/skills):

```bash
bunx skills add arthjean/skills
```

## Third-party skills

Arthur also uses these collections alongside his own.

### [jakubkrehel/skills](https://github.com/jakubkrehel/skills)

Skills by Jakub Krehel that help agents design, build, and review polished interfaces, from UI details and motion to typography, color, accessibility, layout, and interface copy. Arthur uses every skill in this collection.

```bash
bunx skills add jakubkrehel/skills
```

### [mattpocock/skills](https://github.com/mattpocock/skills)

Small, composable engineering skills by Matt Pocock that keep the developer in control: stress-testing a plan, modeling the domain, designing and reviewing code, diagnosing bugs, researching, and running retros. Arthur uses a subset of this collection, so the command installs only those skills.

```bash
bunx skills@latest add mattpocock/skills --skill grilling grill-with-docs domain-modeling codebase-design improve-codebase-architecture code-review diagnosing-bugs research writing-for-agents retro wizard
```

### [cloudflare/security-audit-skill](https://github.com/cloudflare/security-audit-skill)

Cloudflare's skill that turns a coding agent into a security auditor: it maps the codebase, hunts for vulnerabilities, has independent agents try to disprove each finding, and writes a report of what holds up.

```bash
bunx skills add cloudflare/security-audit-skill --skill security-audit
```

## Bonus

Optional skills. They are very useful, but harder to use well than the ones above.

### [cursor/plugins](https://github.com/cursor/plugins)

Cursor's official plugins. Arthur uses a selection of skills that make agents prove their work: creating and maintaining verification and formal verification for a project, checking state machines, turning repeated mistakes into checks, vetting benchmarks, and building and driving CLIs that agents can use reliably.

```bash
bunx skills add cursor/plugins --skill create-formal-verification-skill statechart maintain-formal-verification-skill create-verification-skill maintain-verification-skill correct benchmark-checklist cli-for-agents control-cli
```

## Update

Update every installed skill to its latest version:

```bash
bunx skills update
```

## License

Original material authored by Arthur is available under the MIT License.
