---
name: skill-creator
description: Scaffold, validate, and package a skill folder. Use when asked to create a new skill or package an existing one.
argument-hint: "[skill-name-or-description]"
---

# skill-creator

A skill is a folder with a `SKILL.md` (YAML frontmatter plus Markdown body) and optional `scripts/`, `references/`, and `assets/`. This skill handles the mechanics. How to write the content is a separate concern: read the `writing-for-agents` skill before drafting the body or the description, and treat it as the source of truth on wording.

## Before creating

Ask whether the skill should exist. Each model-invoked skill adds its description to the model's context on every turn, and a crowded roster makes every skill harder to pick. Prefer one of these when it fits: a line in `AGENTS.md`, a plain doc reached by a pointer, or an existing skill extended with one branch. Creating a skill that overlaps an existing one, or overwriting one with the same name, needs the user's say-so.

A skill earns its place for a workflow the model needs only on certain tasks, or for instructions on using a tool or plugin.

## Create

1. Scaffold: `python3 scripts/init_skill.py <skill-name> --path <output-directory>`.
2. Frontmatter: field rules and examples in [Technical rules](references/technical-rules.md). The description states what the skill does in a few words, then one trigger per distinct branch. Shorter is better; the 1024-character limit is a ceiling, not a target.
3. Body: for a single workflow, the steps and their completion criteria. For several workflows, a minimal router pointing to `references/` files so a run reads only what its branch needs. Current models handle nuance well: state invariants, boundaries, and what done looks like, and leave routine choices to the model rather than scripting them. Say explicitly which local actions are authorized without asking. State that explicit user instructions override the skill.
4. Bundle in `scripts/` code that would otherwise be rewritten each run or must be deterministic; in `assets/` templates used in output.
5. Validate: `python3 scripts/quick_validate.py <skill-folder>`. Package for distribution: `python3 scripts/package_skill.py <skill-folder> [output-directory]`.

## Update

Prune before adding. Delete lines the model already obeys by default, forced reads of files the task may not need, encouragement to test that the model already does on its own, and approval gates added to tame an older model. Guidance written for one model can overconstrain another, so keep repository skills model-neutral.

If the skill overtriggers, narrow the trigger to the task rather than the topic. If it undertriggers, use the leading word the user actually types, rather than piling on synonyms.

**Complete when:** `quick_validate.py` passes, the description fits on one or two lines and names only distinct triggers, every `references/` file is linked from `SKILL.md` with the condition for reading it, and no `README.md` sits inside the skill folder.
