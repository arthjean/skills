# PRD Template and Status Contract for /implement-epic and /review-epic

## PRD Template

Contract sections are required: `/implement-epic` and `/review-epic` read them. Optional sections appear only when the evidence gives them real content; leave one out rather than fill it.

```markdown
[PRD]
# PRD: {Feature Name}

## Problem Statement

{What the problem is, who has it, and the evidence for it.}

**Why now:** {What changed that makes this worth solving now}

## Overview

{The proposed solution and how it addresses the problem, including the key decisions and the evidence behind each.}

## Non-Goals

- {What this version will not do, and why}

## Quality Gates

These commands must pass for every user story:
- `{command_1}` - {description}
- `{command_2}` - {description}

## Epics & User Stories

### EP-001: {Epic Title}

{The outcome this epic delivers.}

**Definition of Done:** {Measurable completion condition}

#### US-001: {Story Title}
**Description:** As a {user role}, I want {concrete action} so that {outcome}.

**Priority:** P0
**Dependencies:** None

**Acceptance Criteria:**
- [ ] Given {context}, when {action}, then {verifiable result}
- [ ] Given {failure or edge case}, when {action}, then {explicit behavior}

#### US-002: {Story Title}
**Description:** As a {user role}, I want {concrete action} so that {outcome}.

**Priority:** P1
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**
- [ ] {Criterion}

---

### EP-002: {Epic Title}

{Continue for all epics and stories.}

## Files NOT to Modify

{Required when a codebase exists.}
- `path/to/file.ext`: {reason}
[/PRD]
```

### Optional sections

Add any of these, as a `##` section placed before `## Epics & User Stories` or after it, when the evidence supports it:

- **Goals** or **Success Metrics**: objectives with a baseline, a target, and how each is measured, when those values are known.
- **Target Users**: roles, current workarounds, and pain points the research documented.
- **Research Findings**: the findings that shaped the PRD, each with its source link.
- **Assumptions & Constraints**: what the PRD believes but has not validated, and the hard constraints it must respect.
- **Functional Requirements** (`FR-NN`) and **Non-Functional Requirements**: only requirements with a source; a non-functional target carries a number only when evidence or the user supplies one.
- **Edge Cases & Error States**: scenario, trigger, expected behavior, user message.
- **Risks & Mitigations**: risk, probability, impact, mitigation.
- **Technical Decisions**: each decision with the options considered and the evidence for the choice.
- **Open Questions**: what only the user or a later validation can settle, and what depends on it.

---

## Format Rules for Downstream Compatibility

### Story ID Format
- Stories: `US-NNN`: zero-padded three digits, sequential across all epics
- Epics: `EP-NNN`: zero-padded three digits

### Heading Hierarchy
- `#`: PRD title
- `##`: Top-level sections (the contract sections Problem Statement, Overview, Non-Goals, Quality Gates, Epics & User Stories, Files NOT to Modify, plus any optional section)
- `###`: Epic headings within Epics & User Stories
- `####`: Individual story headings within an epic

### Acceptance Criteria Format
- GitHub Flavored Markdown task list: `- [ ] criterion`
- One atomic, independently verifiable fact per checkbox
- Use Given/When/Then format where applicable
- Never include quality gate commands in criteria

### Quality Gates Section
- Listed once, applies to all stories
- Commands wrapped in backticks: `` `command` ``
- Followed by dash and description: `` `command` - description ``

### PRD Wrapper
- `[PRD]` on its own line before the document
- `[/PRD]` on its own line after the document
- These markers let external parsing tools extract the document

### Story Metadata
Each story carries inline metadata:
- `**Priority:**`: P0 (must have), P1 (should have), P2 (could have)
- `**Dependencies:**`: "None" or "Blocked by US-NNN, US-NNN"

### File Naming
- PRD: `./tasks/prd-{feature-name-kebab-case}.md`
- Status: `./tasks/prd-{feature-name-kebab-case}-status.json`

---

## Status File Schema

```json
{
  "prd": {
    "file": "tasks/prd-{name}.md",
    "title": "{Feature Name}",
    "created_at": "{YYYY-MM-DD}",
    "status": "DRAFT | READY | IN_PROGRESS | IN_REVIEW | DONE"
  },
  "epics": [
    {
      "id": "EP-001",
      "title": "{Epic Title}",
      "status": "TODO | IN_PROGRESS | IN_REVIEW | DONE | BLOCKED",
      "priority": "P0 | P1 | P2",
      "stories_total": 4,
      "stories_done": 0
    }
  ],
  "stories": [
    {
      "id": "US-001",
      "title": "{Story Title}",
      "epic": "EP-001",
      "status": "TODO | IN_PROGRESS | IN_REVIEW | DONE | BLOCKED | CANCELLED",
      "priority": "P0 | P1 | P2",
      "blocked_by": [],
      "started_at": null,
      "completed_at": null,
      "reviewed_at": null
    }
  ]
}
```

### Status Transitions

```
TODO → IN_PROGRESS → IN_REVIEW → DONE
                          ↑          |
                          └──────────┘   downgrade on disproven completion

Any → BLOCKED → IN_PROGRESS
Any → CANCELLED
```

Ownership is strict: `/implement-epic` owns `TODO` → `IN_PROGRESS` → `IN_REVIEW` and never writes `DONE`. `/review-epic` alone writes `DONE` and the `DONE` → `IN_REVIEW` downgrade. Either skill writes `BLOCKED` for a story it cannot move forward. Certification never comes from the skill that wrote the code.

- `TODO` → `IN_PROGRESS`: when `/implement-epic` starts the story
- `IN_PROGRESS` → `IN_REVIEW`: when `/implement-epic` has implemented the story, wired it into a real execution path, and passed its final validation bundle. Implementation is complete but uncertified.
- `IN_REVIEW` → `DONE`: when `/review-epic` proves every criterion from a real entry point, required gates pass, and no blocking finding remains
- `DONE` → `IN_REVIEW`: when `/review-epic` disproves completion or leaves required manual verification
- Any → `BLOCKED`: when a dependency, irreversible decision, unresolvable wiring gap, or repeated technical failure prevents further work
- `BLOCKED` → `IN_PROGRESS`: when the blocker is resolved and `/implement-epic` resumes the story
- Any → `CANCELLED`: manual decision

`/implement-epic` leaves `completed_at` unset; it belongs to certification. `/review-epic` sets `reviewed_at` on every reviewed, non-cancelled child story, sets `completed_at` on newly certified stories, and then recalculates story, epic, and PRD roll-ups.

### Epic Status Roll-up

- `TODO`: no stories started
- `IN_PROGRESS`: at least one story started, at least one non-cancelled story below `IN_REVIEW`
- `IN_REVIEW`: every non-cancelled story is `IN_REVIEW` or `DONE`, and at least one is not `DONE`
- `DONE`: all stories DONE or CANCELLED, written only by `/review-epic`
- `BLOCKED`: at least one non-cancelled story is BLOCKED and no further progress is possible

### PRD Status

- `DRAFT`: during research and writing
- `READY`: written and reviewed against the rubric, ready for implementation
- `IN_PROGRESS`: at least one story started
- `IN_REVIEW`: every epic is `IN_REVIEW` or `DONE`, and at least one is not `DONE`
- `DONE`: all epics DONE

---

## Relationship to Other Skills

```
/write-prd                    → produces PRD + status.json
     |
     v
/implement-epic [prd] [EP-NNN]   → implements and wires one epic through ordered story slices, hands off at IN_REVIEW
     |
     v
/review-epic [prd] [EP-NNN]      → proves wiring, audits, remediates, certifies DONE, updates status roll-ups
     |
     v
/security-review                  → optional standalone security audit outside the epic workflow
```

The status.json file is the shared state between all skills. Each skill reads it to understand progress and updates it after completing its work.
