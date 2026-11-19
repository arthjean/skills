---
model: opus
name: write-prd
description: "Research a feature and write its PRD autonomously: epics, stories, acceptance criteria, quality gates, and a JSON status tracker. Use when asked to write a PRD or spec out a feature."
argument-hint: "[feature description]"
---

# write-prd

Write a PRD for: $ARGUMENTS

Research the domain and the codebase, make every design decision yourself from that evidence, and write `./tasks/prd-[feature-name].md` plus `./tasks/prd-[feature-name]-status.json`. The run is autonomous: where a question would normally go to the user, answer it from the evidence and log the rationale, so the user reviews a finished PRD rather than a questionnaire. Stop early only when `$ARGUMENTS` names no feature to specify, printing `Usage: /write-prd [feature description]`. Explicit user instructions override anything below.

Print `[Phase N/6] NAME` before each phase: INTAKE, RESEARCH, DECIDE, STRUCTURE, WRITE, FINALIZE.

## Phase 1: INTAKE

Extract the domain, research keywords, implied users, and implied scope. Detect the stack from its manifests, and read existing PRDs (`tasks/prd-*.md`, `docs/prd*.md`, `specs/*.md`) for format conventions and overlap.

**Fast mode** applies when the scope is likely under 3 stories (a single endpoint, config change, or isolated component): web research only in Phase 2, and one merged decision pass in Phase 3 that still covers edge cases, quality gates, and the devil's advocate.

## Phase 2: RESEARCH

Research is mandatory and precedes every decision. Each helper runs from its brief in [Brainstorm protocols](references/brainstorm-protocols.md), which carries its budget.

1. `web-researcher` with the web research brief. Wait for it, then compress its findings into the research brief format.
2. In one message, so they run in parallel: `agent-explorer` with the codebase brief and the compressed research, when a codebase exists; `docs-researcher` with the documentation brief, when research names libraries.
3. Synthesize: competitive options, technical trade-offs, feature expectations, risks, codebase constraints. A finding that will drive a decision keeps its source; a claim that arrives without one does not drive a decision.

A failed helper does not stop the run. Without web research, fall back to model knowledge and note the reduced confidence; without exploration, mark codebase constraints unverified; without documentation, use official web documentation and note the gap. With no codebase, skip exploration and omit Files NOT to Modify.

## Phase 3: DECIDE

Print the research brief, then decide each area and log it:

```
### {Area}: {Decision}
**Options considered:** {A, B, C}
**Chosen:** {X}, because {evidence from research or codebase}
```

Every option traces to a research finding or a codebase fact.

- **Vision and scope**: positioning among the competitor approaches, whether the market gap is a v1 differentiator, the primary user, and the MVP drawn from research's minimum user expectations.
- **Technical**: architecture, data handling, and stack aligned with the existing codebase unless research gives a concrete reason to depart; a mitigation for each security risk research raised, built now or tracked as its own story.
- **Prioritization**: MoSCoW for every capability. Must = minimum viable from research, Should = competitive parity, Could = differentiators, Won't = outside v1.
- **Edge cases**: the relevant categories from the edge-case list in the protocols, each with a feature-specific example.
- **Quality gates**: the commands the project already runs, taken from its scripts and CI config.
- **Devil's advocate**: challenge the plan on each axis in the protocols, and log how each surviving concern is handled.

## Phase 4: STRUCTURE

- **Epics**: 2-6, ordered by priority with Must Have first, each with a measurable definition of done and 2-8 stories (more than 8: split the epic).
- **Stories** via SPIDR: Spike (research or validation, last resort), Path (alternate user flows), Interface (technology layer or progressive UI polish), Data (a subset of data types first), Rules (relax business rules, restore them in follow-on stories). Each story gets a `US-NNN` ID, a title, an "As a... I want... so that..." description, `- [ ]` acceptance criteria with at least one unhappy path, a priority (P0, P1, P2), dependencies, and a size, and is completable in one agent session.
- **Edge cases**: a dedicated story for complex ones, criteria on existing stories for simple ones.
- **Dependencies**: which stories block others, which run in parallel, and cross-epic links.
- **Sizes**: XS 1 (config change, single file), S 2 (CRUD endpoint, simple component), M 3 (business logic across files), L 5 (multiple integrations), XL 8 (split further).
- **Validation spikes**: each high-risk assumption becomes "US-NNN: Validate assumption: {X}" with validation criteria.
- **Size cap**: more than 20 stories splits into phased releases or several PRDs.

## Phase 5: WRITE

1. Write the PRD exactly in the format of the [PRD template](references/prd-template.md): `/implement-epic`, `/review-epic`, and other parsers depend on its markers, headings, and IDs.
2. Write the status JSON per the template's status file schema.
3. Run the self-validation checklist from the protocols. Each check cites the PRD section that satisfies it; a check with no citable section fails and gets fixed before saving.
4. Save both files, creating `tasks/` if needed.

## Phase 6: FINALIZE

Set the PRD status to `READY` in the status file. Report, leading with what the user may want to overturn: the decisions resting on the thinnest evidence, the high-risk assumptions, and the open questions. Then the epics and stories table, the quality gates, and the next steps (`/implement-epic`, then `/review-epic`).

**Complete when:** both files are saved, every self-validation check cites its section, and the status JSON is valid and lists every epic and story in the PRD.

## Sources

Tuned against [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/) (Anthropic, 2026-07-24), [Getting the most out of Opus 5.5](https://claude.dev/blog/getting-the-most-out-of-opus-5-5/#say-what-done-looks-like-then-let-it-run) (Anthropic, 2026-09-22), and [OpenAI model guidance for GPT-6 Astra](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6-astra).
