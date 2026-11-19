# Brainstorm protocols

Reference for [`write-prd`](../SKILL.md): the helper briefs for Phase 2, the edge-case list and devil's advocate axes for Phase 3, the self-validation checklist for Phase 5, and the research brief format. Every decision is the skill's own; nothing here is put to the user.

## Contents

- [Helper briefs](#helper-briefs)
- [Edge-case categories](#edge-case-categories)
- [Devil's advocate axes](#devils-advocate-axes)
- [Self-validation checklist](#self-validation-checklist)
- [Research brief format](#research-brief-format)

## Helper briefs

### Web research

```text
Research this domain to inform a Product Requirements Document.

Feature: {user_feature_description}
Domain: {detected_domain}

Cover, in priority order:
1. Competitive landscape: 3-5 comparable products, how each approaches the feature, what users praise and criticize.
2. Best practices: authoritative guides, patterns, or frameworks for building it.
3. Technical patterns: common frameworks, libraries, and architectures, with rationale.
4. User expectations: the minimum viable set, standard capabilities, delighters.
5. Security: OWASP-relevant risks for this domain and recommended mitigations.
6. Common pitfalls: frequent mistakes and failure modes.
7. Trends: emerging standards or upcoming changes that should shape the design.

Budget: 4-6 targeted searches, primary and current sources first. If results are thin, return what you have and name the gaps. Return one section per item above, then Sources as markdown links. Mark anything you could not confirm and say where you looked.
```

### Codebase exploration

```text
Map the codebase constraints for a planned feature. Read-only.

Feature: {user_feature_description}
Research context: {compressed_web_research}

Find:
1. Stack, framework, and architecture pattern.
2. Similar features already implemented, and how they are structured.
3. The auth pattern, if the feature involves auth.
4. The data layer or schema, if the feature involves data.
5. The routing or API pattern, if the feature adds endpoints.
6. Testing conventions: framework, structure, coverage.
7. Shared utilities, components, or services the feature could reuse.
8. Agent guidance, architecture docs, or coding standards (AGENTS.md, CLAUDE.md, docs/).
9. Existing PRD conventions in tasks/ or docs/.
10. Core infrastructure the feature must not modify.

Budget: 15-20 file reads; map the architecture without reading unrelated implementations. Return sections: Tech Stack (with versions), Architecture Pattern (with file:line), Similar Features, Reusable Components, Constraints, Files NOT to Modify, Existing PRD Conventions.
```

### Documentation lookup

```text
Look up library documentation for a feature being planned, not implemented. Read-only.

Feature: {feature_description}
Libraries: {library_list_with_versions}

For each library: the capabilities it offers for this use case, the patterns official docs recommend, limitations or known issues that affect the design, and setup or configuration to plan for. Budget: 3 documentation calls, most relevant libraries first. Stay on design-relevant facts.
```

## Edge-case categories

Select the categories that apply, each with an example specific to the feature, and mark those research flagged as high-risk for the domain. Complex cases get a dedicated story; simple ones become acceptance criteria on existing stories.

| # | Category | Covers |
|---|----------|--------|
| 1 | Empty states | First-time user with no data |
| 2 | Loading states | What users see during async operations |
| 3 | Error states | API failures, validation errors, timeouts |
| 4 | Network degradation | Slow connection, offline mode |
| 5 | Permission changes | Access revoked mid-session |
| 6 | Concurrent modifications | Two users editing simultaneously |
| 7 | Boundary values | Min and max inputs, zero items, overflow |
| 8 | Undo and reversal | Whether critical actions can be reversed |
| 9 | Interrupted flows | Session timeout, tab close, browser back |
| 10 | External dependencies | Third-party service outages |

## Devil's advocate axes

Challenge the plan on each axis. Every concern that survives gets a mitigation, a validation story, or an explicit acceptance in the decision log:

1. **Risk**: the top risks research raised, and where teams building similar features struggle.
2. **Assumption**: each belief the plan rests on that research does not confirm, or contradicts. High-risk ones become validation spikes.
3. **Scope**: the total effort; more than 20 stories means phased releases.
4. **Edge-case coverage**: the categories left out, and why each does not apply.
5. **Missing consideration**: anything research raised that no decision addressed.

## Self-validation checklist

For each check, cite the PRD section that satisfies it. A check with no citable section fails; fix it before saving.

| # | Check |
|---|-------|
| 1 | Problem Statement says why, not only what, and includes "Why now" |
| 2 | Every subjective word ("fast", "simple", "intuitive", "easy") is replaced with a measurable target |
| 3 | Non-Goals lists at least 2 explicit exclusions |
| 4 | Edge Cases & Error States documents at least 2 scenarios |
| 5 | Every user story has at least one unhappy-path acceptance criterion |
| 6 | Success Metrics has baseline, target, and timeframe columns |
| 7 | Every NFR has a specific number (latency in ms, uptime %, concurrent users) |
| 8 | Target Users includes pain points and current workarounds |
| 9 | Risks & Mitigations rates probability and impact |
| 10 | Two engineers reading the PRD independently would build the same thing |
| 11 | No story exceeds XL (8 points) |
| 12 | Total stories are 20 or fewer, or explicitly phased into several releases |
| 13 | Assumptions lists what the PRD believes but has not validated |
| 14 | Technical Considerations are framed as questions for engineering input, not mandates |
| 15 | Changelog has the initial version entry |

## Research brief format

The Phase 2 synthesis, printed at the start of Phase 3. Under 300 words.

```markdown
## Research Brief

### Competitors
- {Name}: {approach}; {strength}, {weakness}

### Best Practices
1. {Practice} ({source})

### Technical Recommendations
- Stack: {recommended}
- Pattern: {recommended}
- Libraries: {lib1} (v{x}), {lib2} (v{y})

### Risks
- {Risk}: {mitigation}

### User Expectations (minimum)
- {Feature}

### Codebase Constraints
- {Constraint} (`file:line`), when a codebase exists
```
