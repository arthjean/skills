---
model: opus
name: write-prd
description: "Research a feature and write its PRD autonomously: epics, stories, acceptance criteria, quality gates, and a JSON status tracker. Use when asked to write a PRD or spec out a feature."
argument-hint: "[feature description]"
---

# write-prd

Write a PRD for: $ARGUMENTS

Research the domain and the codebase, make every design decision yourself from that evidence, and write `./tasks/prd-[feature-name].md` plus `./tasks/prd-[feature-name]-status.json`. The run is autonomous: where a question would normally go to the user, answer it from the evidence and record the rationale, so the user reviews a finished PRD rather than a questionnaire. Stop early only when `$ARGUMENTS` names no feature to specify, printing `Usage: /write-prd [feature description]`. Explicit user instructions override anything below.

How you get there is yours to judge. What follows is what the PRD must rest on, the contract its readers depend on, and what done means.

## Evidence

Every decision traces to a research finding or a codebase fact. A finding that drives a decision keeps its source; a claim that arrives without one does not drive a decision. A number (target, limit, latency) comes from evidence or a stated requirement, or is listed as an assumption. Never invent one to fill a slot.

Scale the research to the feature. A change confined to an established pattern may need only the codebase; a new product surface needs the domain, comparable approaches, and library documentation too. The helpers are `agent-websearch`, `agent-explore`, and `agent-docs`, briefed from the [research briefs](references/brainstorm-protocols.md#research-briefs); do the work directly when a helper is unavailable or the question is small. Read existing PRDs (`tasks/prd-*.md`, `docs/prd*.md`, `specs/*.md`) for conventions and overlap.

When a source of evidence fails, keep going and say so in the PRD: model knowledge in place of web research lowers confidence, an unexplored codebase leaves constraints unverified.

## Decisions

Decide what the feature needs decided: scope and non-goals, the technical approach (aligned with the existing codebase unless evidence gives a concrete reason to depart), a mitigation for each security risk the research raised, the edge cases that matter, and the quality gates, taken from the commands the project's scripts and CI already run.

Before writing, challenge the plan: the risks research raised, the beliefs it rests on that research does not confirm, anything research raised that no decision addressed. The [challenge prompts](references/brainstorm-protocols.md#challenge-prompts) list angles that are easy to miss. A high-risk unconfirmed assumption becomes a validation story.

## Stories

`/implement-epic` works one epic per run and `/review-epic` certifies it, so:

- Each epic delivers an outcome on its own and has a measurable definition of done.
- Each story is completable in one agent session and states its dependencies.
- Each acceptance criterion is one atomic fact, verifiable from a real entry point, and every story has at least one unhappy-path criterion.
- A complex edge case gets its own story; a simple one becomes a criterion on an existing story.

Let the feature set the shape: a small feature is one epic with a few stories, and a large one takes as many epics as it needs.

## Write and check

1. Write the PRD from the [PRD template](references/prd-template.md). Its contract sections, IDs, criteria format, and status schema are what `/implement-epic` and `/review-epic` read: keep them as specified. Every other section is optional and appears only when the evidence gives it real content.
2. Write the status JSON per the template's schema and save both files, creating `tasks/` if needed.
3. Hand both files to a fresh read-only subagent with the [review rubric](references/brainstorm-protocols.md#review-rubric): an author rereading its own PRD is too lenient. Fix what it reports and recheck only those items.

## Finish

Set the PRD status to `READY`. Report, leading with what the user may want to overturn: the decisions resting on the thinnest evidence (with the options considered), the high-risk assumptions, and the open questions. Then the epics and stories, the quality gates, and the next steps (`/implement-epic`, then `/review-epic`).

**Complete when:** both files are saved, the rubric review has no open failure, and the status JSON is valid and lists every epic and story in the PRD.
