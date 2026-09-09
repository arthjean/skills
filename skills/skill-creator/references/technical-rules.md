# Technical Rules Reference

## File Structure

```
your-skill-name/
├── SKILL.md                # Required - main skill file
├── scripts/                # Optional - executable code (Python, Bash, etc.)
│   ├── process_data.py
│   └── validate.sh
├── references/             # Optional - documentation loaded as needed
│   ├── api-guide.md
│   └── examples/
└── assets/                 # Optional - templates, fonts, icons used in output
    └── report-template.md
```

## Critical Naming Rules

### SKILL.md Naming
- Must be exactly `SKILL.md` (case-sensitive)
- No variations accepted: SKILL.MD, skill.md, Skill.md all FAIL

### Skill Folder Naming
- Use kebab-case: `notion-project-setup`
- No spaces: `Notion Project Setup` FAILS
- No underscores: `notion_project_setup` FAILS
- No capitals: `NotionProjectSetup` FAILS

### No README.md
- Do NOT include README.md inside the skill folder
- All documentation goes in SKILL.md or references/
- When distributing via GitHub, the repo-level README is separate from the skill folder

## YAML Frontmatter Field Requirements

### name (required)
- kebab-case only
- No spaces or capitals
- Should match folder name
- Max 64 characters
- Cannot start/end with hyphen or contain consecutive hyphens
- Cannot include "claude" or "anthropic" (reserved)

### description (required)
- As short as possible while making clear when the skill should fire: what it does in a few words, then the distinct trigger branches
- No XML tags (< or >)
- Hard limit 1024 characters, but a good description is far below it: every character sits in context on every turn, and Codex truncates descriptions when too many skills are loaded
- Triggers name the task, not the topic. A description that fires on any mention of a topic pushes the model to load instructions that do not help
- Cut identity the body already carries, and cut synonyms that rename one trigger

**Good examples:**

```yaml
# Fires only on the task, not on every database mention
description: Write and apply a schema migration. Use when the user asks for a migration or a schema change.

# Two distinct branches, one trigger each
description: Certify one implemented PRD epic and write its final status. Use when asked to review, certify, or recheck an epic.
```

**Bad examples:**

```yaml
# Too vague
description: Helps with projects.

# Topic trigger: loads on anything database-related
description: Database expert. Use whenever the user works with databases, SQL, schemas, queries, ORMs, or data.

# Pick-me energy and stacked synonyms: overtriggers and crowds out neighbors
description: Use this skill ALWAYS for any API question, even when you think you know the answer. Handles setup, configuration, "how do I", debugging, migration, CLI usage, and more.
```

### license (optional)
- Use if making skill open source
- Common: MIT, Apache-2.0

### compatibility (optional)
- 1-500 characters
- Indicates environment requirements: intended product, required system packages, network access needs

### metadata (optional)
- Any custom key-value pairs
- Suggested: author, version, mcp-server
```yaml
metadata:
    author: ProjectHub
    version: 1.0.0
    mcp-server: projecthub
```

### model (optional)
- Override the model used when this skill runs
- Common values: `opus`, `sonnet`, `haiku`
- Example: `model: opus` for complex multi-agent pipelines
- If omitted, the skill uses the current session's model

### argument-hint (optional)
- Placeholder text shown in autocomplete to indicate expected arguments
- Example: `argument-hint: "[file-or-folder] [--perf|--clean]"`
- Use with `$ARGUMENTS`, `$0`, `$1` etc. in the skill body

### context (optional)
- Set to `fork` to run the skill in an isolated subagent context (fresh window, no conversation history)
- The skill content becomes the agent's task prompt
- Only use for skills with explicit actionable instructions : background-knowledge skills without a clear task will return without meaningful output
- Example: `context: fork`

### agent (optional)
- Which agent type to use when `context: fork` is set
- Available: `Explore`, `Plan`, `general-purpose`, or custom agent names from `.claude/agents/`
- Example: `agent: Explore`

### disable-model-invocation (optional)
- Set to `true` to prevent Claude from auto-triggering this skill
- The skill becomes manual-only (user must type `/skill-name`)
- Use for skills with side effects: `/deploy`, `/commit`, `/push`
- Example: `disable-model-invocation: true`

### user-invocable (optional)
- Set to `false` to hide the skill from the `/` menu
- Claude can still auto-trigger it when relevant
- Use for background domain knowledge Claude should apply passively
- Example: `user-invocable: false`

### allowed-tools (optional)
- List of tools the skill is allowed to use
- Restricts the tool surface to only what the skill needs
- Supports glob patterns for Bash: `Bash(git *)`, `Bash(npm *)`
- Example: `allowed-tools: Read, Grep, Glob, Bash(git log *)`
- Use for read-only audit skills to prevent accidental writes

## Security Restrictions

### Forbidden in frontmatter:
- XML angle brackets (< >)
- Skills with "claude" or "anthropic" in name (reserved)

**Why:** Frontmatter appears in Claude's system prompt. Malicious content could inject instructions.

## Resource Guidelines

### Scripts (`scripts/`)
- Include when the same code is rewritten repeatedly or deterministic reliability is needed
- Scripts may be executed without loading into context (token efficient)
- Test all scripts by running them before packaging

### References (`references/`)
- Include for documentation Claude should reference while working
- Keep SKILL.md lean; move detailed information here
- If files are large (>10k words), include grep search patterns in SKILL.md
- Structure longer files (>100 lines) with table of contents
- Avoid duplication between SKILL.md and references

### Assets (`assets/`)
- Files used in output, not loaded into context
- Templates, images, icons, boilerplate code, fonts
- Separate output resources from documentation

## Frontmatter Validation Checklist

1. Starts with `---` delimiter
2. Ends with `---` delimiter
3. Contains `name` field (kebab-case string)
4. Contains `description` field (string, < 1024 chars)
5. No XML tags in any field
6. No unknown fields (only: name, description, model, argument-hint, context, agent, disable-model-invocation, user-invocable, license, allowed-tools, compatibility, metadata)
7. Valid YAML syntax (proper quoting, no unclosed quotes)
