# PRD Template - Exact Format for /implement-epic and /review-epic Compatibility

## Complete PRD Template

```markdown
[PRD]
# PRD: {Feature Name}

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.0 | {YYYY-MM-DD} | {author} | Initial draft |

## Problem Statement

{Specific, numbered problem description. Answer: What is the problem? Who has it? Why now?}

1. {Problem 1 — specific pain point with evidence from research or user data}
2. {Problem 2}

**Why now:** {What has changed that makes solving this urgent — market shift, user demand, competitive pressure, compliance deadline}

## Overview

{2-3 paragraphs: What is the proposed solution? How does it address the problems above?
Include the key decisions made during brainstorming.}

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|---------------|----------------|
| {Measurable objective 1} | {target} | {target} |
| {Measurable objective 2} | {target} | {target} |
| {Measurable objective 3} | {target} | {target} |

## Target Users

### {User Role A}
- **Role:** {Who they are}
- **Behaviors:** {How they currently work, what tools they use}
- **Pain points:** {What frustrates them today}
- **Current workaround:** {How they solve this problem now}
- **Success looks like:** {What outcome would delight them}

### {User Role B}
- **Role:** {Who they are}
- **Behaviors:** {How they currently work}
- **Pain points:** {What frustrates them}
- **Current workaround:** {How they solve this now}
- **Success looks like:** {What outcome would delight them}

## Research Findings

Key findings that informed this PRD:

### Competitive Context
- {Competitor A}: {what they offer, how we differ}
- {Competitor B}: {what they offer, how we differ}
- **Market gap:** {unmet need we're addressing}

### Best Practices Applied
- {Practice 1 from research that shaped our approach}
- {Practice 2}

*Full research sources available in project documentation.*

## Assumptions & Constraints

### Assumptions (to validate)
- {Assumption 1 — what we believe to be true, based on {evidence}}
- {Assumption 2}

### Hard Constraints
- {Constraint 1 — e.g., must work with existing auth system}
- {Constraint 2 — e.g., legal/compliance requirement}
- {Constraint 3 — e.g., must ship before {date}}

## Quality Gates

These commands must pass for every user story:
- `{command_1}` - {description}
- `{command_2}` - {description}

{For UI stories, additional gates:}
- {visual verification instruction}

## Epics & User Stories

### EP-001: {Epic Title}

{1-2 sentence epic description. What business outcome does this epic deliver?}

**Definition of Done:** {When is this epic complete?}

#### US-001: {Story Title}
**Description:** As a {user role}, I want {concrete action} so that {measurable outcome}.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** None

**Acceptance Criteria:**
- [ ] Given {context}, when {action}, then {verifiable result}
- [ ] Given {edge case}, when {action}, then {explicit behavior}
- [ ] {Additional atomic criterion}

#### US-002: {Story Title}
**Description:** As a {user role}, I want {concrete action} so that {measurable outcome}.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**
- [ ] {Criterion}
- [ ] {Criterion}

---

### EP-002: {Epic Title}

{Epic description.}

**Definition of Done:** {Completion criteria}

#### US-003: {Story Title}
**Description:** As a {user role}, I want {action} so that {outcome}.

**Priority:** P1
**Size:** S (2 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**
- [ ] {Criterion}
- [ ] {Criterion}

#### US-004: {Story Title}
...

---

{Continue for all epics and stories...}

## Functional Requirements

- FR-01: {The system must...}
- FR-02: {When a user..., the system must...}
- FR-03: {The system must NOT...}

## Non-Functional Requirements

IMPORTANT: Every NFR must have a specific, measurable number. No subjective adjectives.

- **Performance:** {e.g., "P95 API latency <200ms", "Dashboard loads in <800ms on 3G", "Supports 500 concurrent users at launch"}
- **Security:** {e.g., "OWASP Top 10 compliant", "All PII encrypted at rest (AES-256)", "Session timeout after 30 min inactivity"}
- **Accessibility:** {e.g., "WCAG 2.1 AA compliant", "Full keyboard navigation", "Screen reader compatible"}
- **Scalability:** {e.g., "Supports 10,000 concurrent users by Month 6", "Database handles 1M records without degradation"}
- **Reliability:** {e.g., "99.9% uptime SLA", "Auto-retry 3x on transient failures", "Graceful degradation when {service} is down"}

## Edge Cases & Error States

Systematic coverage of unhappy paths. Evidence shows earlier defect discovery significantly reduces cost (Boehm 1981, NIST 2002).

| # | Scenario | Trigger | Expected Behavior | User Message |
|---|----------|---------|-------------------|--------------|
| 1 | {Empty state} | {First-time user, zero data} | {Show onboarding prompt} | "{CTA message}" |
| 2 | {Loading state} | {Async operation in progress} | {Show skeleton/spinner} | — |
| 3 | {Error state} | {API failure, validation error} | {Show actionable error} | "{Error message with next step}" |
| 4 | {Network degradation} | {Slow/offline connection} | {Graceful degradation behavior} | "{Offline message}" |
| 5 | {Boundary value} | {Min/max/zero/overflow input} | {Explicit behavior at limits} | "{Validation message}" |
| {N} | {Additional relevant scenarios from Phase 3e} | ... | ... | ... |

## Risks & Mitigations

| # | Risk | Probability | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | {Risk from research} | High/Med/Low | High/Med/Low | {Specific mitigation strategy} |
| 2 | {Technical risk} | High/Med/Low | High/Med/Low | {Mitigation} |
| 3 | {Market/business risk} | High/Med/Low | High/Med/Low | {Mitigation} |

## Non-Goals

Explicit boundaries — what this version does NOT include:

- {What this feature explicitly will NOT do — and why}
- {Feature that is out of scope — and when it might be revisited}
- {Adjacent functionality deferred to future work}

## Files NOT to Modify

{Only include if a codebase exists. Critical for AI agents.}
- `path/to/core/infrastructure.ext` — {reason}
- `path/to/shared/config.ext` — {reason}

## Technical Considerations

Frame as questions for engineering input — not mandates:

- **Architecture:** {Key decision} — recommended: {option A}. Engineering to confirm feasibility.
- **Data Model:** {Schema changes needed?} — {option A} vs {option B}. Trade-off: {description}.
- **API Design:** {New endpoints needed?} — recommended: {approach}. Pagination strategy: cursor-based or offset-based?
- **Dependencies:** {New libraries or services?} — {library} (v{x}) recommended by research. Alternatives: {alt}.
- **Migration:** {Data migration needs?} — backward compatibility requirement: {yes/no}. Rollback plan: {description}.

## Success Metrics

| Metric | Baseline (current) | Target | Timeframe | How Measured |
|--------|-------------------|--------|-----------|-------------|
| {Metric 1} | {current value or "N/A (new)"} | {target} | {Month-1 / Month-6} | {analytics tool, query, or manual} |
| {Metric 2} | {current value} | {target} | {timeframe} | {measurement method} |

## Open Questions

- {Question 1 — who should answer, by when, what depends on this}
- {Question 2 — who should answer, by when, what depends on this}
[/PRD]
```

---

## Format Rules for Downstream Compatibility

### Story ID Format
- Stories: `US-NNN` — zero-padded three digits, sequential across all epics
- Epics: `EP-NNN` — zero-padded three digits

### Heading Hierarchy
- `#` — PRD title
- `##` — Top-level sections (Changelog, Problem Statement, Overview, Goals, Target Users, Research Findings, Assumptions & Constraints, Quality Gates, Epics & User Stories, Functional Requirements, Non-Functional Requirements, Edge Cases & Error States, Risks & Mitigations, Non-Goals, Files NOT to Modify, Technical Considerations, Success Metrics, Open Questions)
- `###` — Epic headings within Epics & User Stories section, or persona headings in Target Users
- `####` — Individual story headings within an epic

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
- These markers are used by ralph-tui and other parsing tools

### Story Metadata
Each story carries inline metadata:
- `**Priority:**` — P0 (must have), P1 (should have), P2 (could have)
- `**Size:**` — XS (1pt), S (2pt), M (3pt), L (5pt), XL (8pt)
- `**Dependencies:**` — "None" or "Blocked by US-NNN, US-NNN"

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
    "status": "DRAFT | READY | IN_PROGRESS | DONE"
  },
  "epics": [
    {
      "id": "EP-001",
      "title": "{Epic Title}",
      "status": "TODO | IN_PROGRESS | DONE",
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
      "size": "XS | S | M | L | XL",
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
TODO → IN_PROGRESS → IN_REVIEW
  |         |              |
  |         └──────────────→ DONE
  └→ BLOCKED
  |
  └→ CANCELLED
```

- `TODO` → `IN_PROGRESS`: when `/implement-epic` starts the matching story slice in Phase 2
- `IN_PROGRESS` → `IN_REVIEW`: when a story still needs manual verification after `/implement-epic` validation
- `IN_PROGRESS` or `IN_REVIEW` → `DONE`: when `/implement-epic` or `/review-epic` proves every story criterion and no manual verification remains
- `DONE` → `IN_REVIEW`: when `/review-epic` disproves completion or leaves required manual verification
- Any → `BLOCKED`: when a dependency, irreversible decision, or repeated technical failure prevents further work
- `BLOCKED` → `TODO`: when blocker is resolved
- Any → `CANCELLED`: manual decision

`/review-epic` sets `reviewed_at` on every reviewed, non-cancelled child story and then recalculates story, epic, and PRD roll-ups.

### Epic Status Roll-up

- `TODO`: no stories started
- `IN_PROGRESS`: at least one story started, not all done
- `DONE`: all stories DONE or CANCELLED

### PRD Status

- `DRAFT`: during brainstorming and writing
- `READY`: user approved, ready for implementation
- `IN_PROGRESS`: at least one story started
- `DONE`: all epics DONE

---

## Relationship to Other Skills

```
/write-prd                    → produces PRD + status.json
     |
     v
/implement-epic [prd] [EP-NNN]   → implements one epic through ordered story slices, updates roll-up status
     |
     v
/review-epic [prd] [EP-NNN]      → reviews and corrects one implemented epic, updates status roll-ups
     |
     v
/security-review                  → optional standalone security audit outside the epic workflow
```

The status.json file is the shared state between all skills. Each skill reads it to understand progress and updates it after completing its work.
