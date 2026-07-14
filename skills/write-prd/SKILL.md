---
model: opus
name: write-prd
description: "Fully autonomous research-informed PRD generator. Researches the domain, makes all design decisions based on research + codebase context, then produces a complete PRD with epics, stories, acceptance criteria, quality gates, and a JSON status tracking file. Invoke with /write-prd [feature description]."
argument-hint: "[feature description]"
---

# write-prd — Research-Informed PRD Generator

Write a PRD for: $ARGUMENTS

## Overview

Fully autonomous research-first PRD generator. Researches the domain, competitors, best practices, and technical landscape, then makes all design decisions autonomously based on research findings + codebase context. Never asks the user questions — decides based on available evidence.

**Outputs:**
1. A complete PRD (`./tasks/prd-[name].md`)
2. A JSON status tracking file (`./tasks/prd-[name]-status.json`)

## Execution Flow

`INTAKE → RESEARCH → DECIDE → STRUCTURE → WRITE → FINALIZE`

Print `[Phase N/6] PHASE_NAME` before each phase. Fully autonomous — no user questions at any phase.

## Phase-by-Phase Execution

### Phase 1 — INTAKE

**1a. Validate input:**

If `$ARGUMENTS` is empty or fewer than 5 words, fail with a clear error: "Usage: /write-prd [feature description]. Provide at least a sentence describing the feature."

**1b. Parse `$ARGUMENTS`:**

Extract: **Domain** (area: auth, payments, UI, data pipeline...), **Keywords** (for web research), **Implied users** (who will use this), **Implied scope** (small feature / full product / platform).

**1c. Scope detection — fast mode check:**

If the implied scope is very small (likely < 3 stories — a single endpoint, config change, or isolated component), switch to FAST MODE:
- Phase 2: web-researcher only (skip agent-explorer and docs-researcher)
- Phase 3: merge Rounds 1-3 into a single focused round, keep Edge Cases + Quality Gates + Devil's Advocate
- Phase 4-6: unchanged

**1d. Detect existing codebase context:**

Run parallel Glob calls: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`. Also check for existing PRDs: `tasks/prd-*.md`, `docs/prd*.md`, `specs/*.md`. If existing PRDs found, read them for format conventions and overlap avoidance.

**GATE:** Domain identified. Input validated. Scope mode determined (standard or fast).

---

### Phase 2 — RESEARCH (mandatory)

Deep research BEFORE asking a single question.

**2a. Spawn web-researcher** with the Web Research Prompt Template from [references/brainstorm-protocols.md](references/brainstorm-protocols.md). Scope guard: 4-6 targeted web searches max, prioritizing primary sources and breadth. If results are thin, proceed and note gaps.

Wait for completion. Compress key findings into the Compressed Research Summary Format from brainstorm-protocols.md (target: < 300 words internal, < 500 words for user presentation).

**2b. Spawn agent-explorer and docs-researcher in parallel (if applicable):**

If codebase detected: read the Codebase Exploration Prompt Template from brainstorm-protocols.md, substitute the feature description and compressed web research, and spawn agent-explorer. **Scope guard:** max 15-20 file reads; map architecture without reading unrelated implementations.

If libraries are identified from research: read the Documentation Lookup Prompt Template from brainstorm-protocols.md and spawn docs-researcher. **Scope guard:** max 3 ctx7 calls; prioritize the most relevant libraries.

Spawn both in a SINGLE message for parallel execution.

**2c. Synthesize research:**

Store the synthesis in the Compressed Research Summary Format from brainstorm-protocols.md. Organize into: Competitive options, Technical trade-offs, Feature expectations, Risk areas, Existing codebase constraints (if applicable).

**GATE:** Research synthesis complete. Must have at least web research results.

---

### Phase 3 — DECIDE (autonomous decision-making)

Make all design decisions autonomously based on research findings + codebase context. Use the question templates from [references/brainstorm-protocols.md](references/brainstorm-protocols.md) as a decision framework — but answer them yourself instead of asking the user.

**3a. Print research summary** — display the compressed research brief (<300 words) for visibility.

**3b. Vision & Scope decisions:** Using the Round 1 framework, decide: competitive positioning (which approach fits best given the codebase), target users (infer from existing code + feature description), scope (MVP features based on research's minimum user expectations). Log each decision with rationale.

**3c. Technical decisions:** Using the Round 2 framework, decide: architecture pattern (align with existing codebase), data handling (match existing patterns), security approach (follow research best practices), tech stack (stay within existing stack unless research strongly recommends otherwise). Log each decision with rationale.

**3d. Prioritization:** Using the Round 3 framework, assign MoSCoW priorities to all identified capabilities. Must Have = minimum viable from research. Should Have = competitive parity. Could Have = differentiators. Won't Have = out of scope for v1.

**3e. Edge Cases & Error States:** Using the Edge Cases template, auto-select relevant categories from the 10 standard categories based on the feature domain. No need to ask — if the feature handles user input, mark validation/boundary/error states as relevant. If it has async operations, mark loading/network states.

**3f. Quality Gates:** Auto-detect from codebase manifests (Cargo.toml → `cargo check && cargo clippy && cargo test`, package.json → detect scripts, etc.).

**3g. Devil's Advocate:** Self-challenge: identify top 3 risks from research, check if any assumptions are unvalidated, flag scope concerns if >20 stories. Log mitigations for each.

**Decision log format** (print for each decision area):
```
### {Area}: {Decision}
**Options considered:** {A, B, C}
**Chosen:** {X} — because {rationale from research/codebase}
```

**GATE:** All decisions logged. Quality gates defined. Scope clear.

---

### Phase 4 — STRUCTURE

Decompose brainstorm output into a formal epic/story hierarchy.

**4a. Define epics:** 2-6 epics from brainstorm decisions. Each epic: 2-8 stories (more than 8 → split). Ordered by priority (Must Have first). Each has a measurable definition of done.

**4b. Decompose stories per epic** using SPIDR:
1. **S (Spike)** — extract research/validation into separate stories (last resort)
2. **P (Path)** — split by alternate user flows
3. **I (Interface)** — split by technology layer or progressive UI polish
4. **D (Data)** — restrict to subset of data types initially
5. **R (Rules)** — temporarily relax business rules, add back in follow-on stories

Each story gets: `US-NNN` ID, title, description ("As a... I want... so that..."), acceptance criteria (`- [ ]` format), priority (P0/P1/P2), dependencies, size estimate.

**4c. Add edge case coverage** from Phase 3e: dedicated story for complex cases, acceptance criteria on existing stories for simpler cases.

**4d. Map dependencies:** Which stories block others? Which run in parallel? Cross-epic dependencies?

**4e. Estimate sizes:**

| Size | Points | Description |
|------|--------|-------------|
| XS | 1 | Config change, single file |
| S | 2 | CRUD endpoint, simple component |
| M | 3 | Feature with business logic, multiple files |
| L | 5 | Complex feature, multiple integrations |
| XL | 8 | Should probably be split further |

**4f. Convert high-risk assumptions to validation stories:** For each assumption marked HIGH risk in the brainstorm, create a spike story: "US-NNN: Validate assumption: {X}" with acceptance criteria for validation.

**GATE:** All epics defined. All stories have IDs, descriptions, criteria, priorities, dependencies. Edge cases covered.

---

### Phase 5 — WRITE

**5a. Write the PRD:** Follow [references/prd-template.md](references/prd-template.md) exactly, filling each section from brainstorm output. Wrap in `[PRD]...[/PRD]` markers.

**5b. Write the status JSON:** Follow the Status File Schema in [references/prd-template.md](references/prd-template.md), including Status Transitions and Epic/PRD Status Roll-up rules.

**5c. Self-validate (mandatory before saving):**

Run the PRD Self-Validation Checklist from [references/brainstorm-protocols.md](references/brainstorm-protocols.md). Think step-by-step through each item. For each: cite the specific PRD section that satisfies it. If you cannot cite a specific section, the check FAILS — fix before saving.

**5d. Save both files:**

```
./tasks/prd-[feature-name].md          — the PRD
./tasks/prd-[feature-name]-status.json — the status tracker
```

Create `tasks/` if it doesn't exist.

**GATE:** Both files written. All validation checks pass.

---

### Phase 6 — FINALIZE

**6a. Mark as READY:** Set status to `"READY"` in the JSON file.

**6b. Display summary** - print epics/stories table, quality gates, and next steps (`/implement-epic`, `/review-story`).

**GATE:** Files saved. Summary displayed.

---

## Hard Rules

1. IDs: `US-NNN`, `EP-NNN`. PRD in `[PRD]...[/PRD]` markers. Criteria: `- [ ]` format. Quality gates listed ONCE.
2. Research (Phase 2) MANDATORY. Edge cases, quality gates, devil's advocate steps MANDATORY in Phase 3.
3. Every story: at least one unhappy-path criterion. Independently completable in one AI agent session.
4. No subjective NFRs — measurable numbers only. Success Metrics: baseline + target + timeframe.
5. Self-validation (Phase 5c) must pass before saving. Problem Statement mandatory.
6. If >20 stories, split into multiple PRDs or phased releases.
7. Decide autonomously at every gate — never ask the user. Log decision rationale visibly.

## Error Handling

| Error | Action |
|---|---|
| web-researcher fails | Continue with model knowledge, note reduced quality |
| agent-explorer fails | Skip codebase-specific decisions, note constraints unverified |
| docs-researcher fails | Rely on official web documentation and note the gap |
| >20 stories | Auto-split into phased releases |
| No codebase detected | Skip explore, omit Files NOT to Modify |
| Empty $ARGUMENTS | Fail with usage error |

## References

- [Brainstorm Protocols](references/brainstorm-protocols.md) — agent prompt templates, question round templates, self-validation checklist, compressed research format
- [PRD Template](references/prd-template.md) — exact PRD format, status file schema, downstream compatibility rules
