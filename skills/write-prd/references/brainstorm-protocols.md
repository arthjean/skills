# Brainstorm Protocols — Agent Prompts, Question Templates, and Validation

This file is the single source of truth for all templates used by `/write-prd`. SKILL.md orchestrates; this file provides the content.

## Table of Contents

- [Phase 2 — Agent Prompts](#phase-2--agent-prompts)
- [Phase 3 — Question Templates](#phase-3--research-informed-question-patterns)
- [PRD Self-Validation Checklist](#prd-self-validation-checklist-phase-5c)
- [Compressed Research Summary Format](#compressed-research-summary-format)

---

## Phase 2 — Agent Prompts

### 2a — Web Research Prompt Template

```
Research the following domain to inform a Product Requirements Document.

## Feature Description
{user_feature_description}

## Domain
{detected_domain}

## Research Priorities (IN ORDER)
1. **Competitive landscape** — Find 3-5 competitors or comparable products that offer this type of feature. For each: what they offer, how they approach it, what users praise/criticize.

2. **Best practices** — How do industry leaders recommend building this type of feature? What are the authoritative guides, patterns, or frameworks?

3. **Technical patterns** — What frameworks, libraries, or architectures are commonly used? What's the recommended stack for this type of feature?

4. **User expectations** — What do users in this domain typically expect? What's the minimum viable feature set? What delights users?

5. **Security considerations** — What are the OWASP-relevant risks? What security patterns are recommended for this domain?

6. **Common pitfalls** — What mistakes do teams commonly make when building this? What are the failure modes?

7. **Market trends** — Is this domain evolving? Are there emerging standards or upcoming changes that should influence the design?

## Search Strategy
- Search for "{feature_type} competitors comparison {year}"
- Search for "{feature_type} best practices {tech_stack}"
- Search for "{feature_type} implementation guide"
- Search for "{feature_type} security considerations OWASP"
- Search for "{feature_type} common mistakes"
- Include recent year in searches for freshness

## Output Requirements
Return findings in this structure:

### Competitive Landscape
[For each competitor: name, approach, strengths, weaknesses, unique features]

### Industry Best Practices
[Authoritative recommendations with sources]

### Recommended Technical Patterns
[Frameworks, libraries, architectures with rationale]

### User Expectations
[Minimum feature set, standard capabilities, delighters]

### Security Considerations
[Domain-specific risks and recommended mitigations]

### Common Pitfalls
[Mistakes to avoid, failure modes, anti-patterns]

### Market Trends
[Emerging standards, upcoming changes, trajectory]

### Sources
[All URLs as markdown links]
```

### 2b — Codebase Exploration Prompt Template

```
Explore the codebase to understand the current architecture and constraints for a new feature.

## Planned Feature
{user_feature_description}

## Research Context
{compressed_web_research}

## Exploration Tasks
1. Identify the project's tech stack, framework, and architecture pattern
2. Find similar features already implemented — how were they structured?
3. Identify the authentication/authorization pattern (if the feature involves auth)
4. Check the database schema or data layer (if the feature involves data)
5. Find the routing/API pattern (if the feature adds endpoints)
6. Check testing patterns — framework, structure, coverage conventions
7. Identify shared utilities, components, or services that the new feature could reuse
8. Look for CLAUDE.md, architecture docs, or coding standards
9. Check existing PRDs in tasks/ or docs/ for format conventions
10. Identify files/modules that should NOT be modified (core infrastructure)

## Output Requirements
### Tech Stack
[Languages, frameworks, key dependencies with versions]

### Architecture Pattern
[How the project is organized, with file:line references]

### Similar Features
[How comparable features are implemented, patterns to follow]

### Reusable Components
[Existing code the new feature should use]

### Constraints
[What the architecture supports and doesn't support]

### Files NOT to Modify
[Core infrastructure files that should be protected]

### Existing PRD Conventions
[If PRDs exist, their format and conventions]
```

### 2c — Documentation Lookup Prompt Template

```
Look up documentation for libraries and frameworks relevant to a planned feature.

## Feature
{feature_description}

## Libraries to Look Up
{library_list_with_versions}

## Focus
We're PLANNING a feature, not implementing yet. Look up:
1. What capabilities these libraries provide for our use case
2. Recommended patterns and architecture from official docs
3. Limitations or known issues that would affect our design
4. Configuration or setup requirements we should plan for

## ctx7 CLI Protocol
Two-step process in the available shell:
1. bunx ctx7@latest library {library_name} "{query}"  — resolve library ID
2. bunx ctx7@latest docs {library_id} "{query}"       — fetch documentation

## Important
- Use ctx7 CLI two-step protocol: library first, then docs
- Maximum 3 ctx7 calls total (cost/time budget — prioritize the most relevant libraries)
- Do NOT modify any files — this is read-only research
- Focus on design-relevant information, not implementation details
```

---

## Phase 3 — Research-Informed Question Patterns

### The Core Principle

Every question MUST follow this pattern:

```
Based on our research, [specific finding].

{N}. [Question derived from finding]
   A. [Option informed by research] — [pro from research], [con from research]
   B. [Alternative from research] — [pro], [con]
   C. [Hybrid/custom option]
   D. Other: [describe]
```

**NEVER ask** vague, ungrounded questions ("What do you want to build?", "How should we handle auth?"). **ALWAYS** trace each option to a research finding.

---

### Round 1 — Vision & Scope

**Purpose:** Establish WHAT we're building and WHY, informed by competitive landscape.

```markdown
## Research Summary

Before we start brainstorming, here's what I found about {domain}:

### Competitive Landscape
- **{Competitor A}:** {1-2 sentence summary}
- **{Competitor B}:** {1-2 sentence summary}
- **Market gap:** {unmet need none address well}

### Key Best Practices
- {Practice 1}
- {Practice 2}

### Notable Risk
- {Top risk from research}

*Based on {N} sources — full details available on request.*

---

Now let me ask some questions to shape YOUR vision:

1. {Competitor A} focuses on {approach A}, while {Competitor B} emphasizes {approach B}.
   What resonates most with YOUR product vision?
   A. {Approach A} — {pro: from research}, {con: from research}
   B. {Approach B} — {pro: from research}, {con: from research}
   C. Combine elements of both — specifically {suggested hybrid}
   D. Different direction — [describe]

2. Research shows users in {domain} expect at minimum: {X, Y, Z}.
   Which are must-haves for YOUR first version?
   A. All of them — full feature parity
   B. {X and Y} only — defer {Z} to v2
   C. {X} only — start minimal, iterate fast
   D. Different priorities — [specify]

3. Who is the primary user of this feature?
   A. {User type A from research — e.g., "end users (consumers)"}
   B. {User type B — e.g., "business admins"}
   C. Both, with different experiences
   D. Other — [specify]

4. The market gap we identified is: {gap}.
   Is addressing this gap a priority?
   A. Yes — make it a core differentiator
   B. Interesting but not for v1
   C. No — focus on proven features first
```

---

### Round 2 — Technical Decisions

**Purpose:** Lock in HOW we'll build it, informed by technical research and codebase analysis.

```markdown
Based on your vision answers and our technical research:

1. For {technical decision A}, research recommends:
   A. {Pattern A} — used by {companies/frameworks}, {pro}, {con}
   B. {Pattern B} — recommended by {source}, {pro}, {con}
   C. {Pattern C} — emerging approach, {pro}, {con}
   {If codebase exists: "Note: your current codebase uses {existing pattern}."}

2. For data handling, {research finding about data patterns}:
   A. {Approach A} — {when it's best}
   B. {Approach B} — {when it's best}
   C. Align with existing codebase pattern: {pattern}

3. Security research found {domain-specific risk}:
   A. {Mitigation A from research} — industry standard, {trade-off}
   B. {Mitigation B from research} — more secure, {trade-off}
   C. Address in a dedicated security story (defer but track)
   D. Other approach

4. {If codebase exists}: Your project uses {tech stack}.
   For this feature, should we:
   A. Stay fully within the existing stack
   B. Add {library from research} for {specific capability}
   C. Evaluate during implementation
```

---

### Round 3 — Scope & Prioritization

**Purpose:** Define boundaries and priorities using MoSCoW informed by research.

```markdown
Based on our discussion, here are the capabilities I've identified.
Rate each using MoSCoW:

| # | Capability | Research Context | Your Priority? |
|---|-----------|-----------------|----------------|
| 1 | {capability} | {who does it, why it matters} | M / S / C / W |
| 2 | {capability} | {research context} | M / S / C / W |
| 3 | {capability} | {research context} | M / S / C / W |
| ... | ... | ... | ... |

M = Must Have (MVP, blocks launch)
S = Should Have (important, not blocking)
C = Could Have (nice-to-have)
W = Won't Have (out of scope)

Additional scoping questions:

1. Based on the Must Haves, I'd suggest {N} epics with ~{M} stories total.
   Does this feel right for your timeline?
   A. Yes, proceed
   B. Too ambitious — reduce scope
   C. Too small — add more
   D. Let me see the breakdown first

2. Should we plan for {future consideration from research}?
   A. Yes, architect for it now (costs more upfront)
   B. No, build for current needs only (may need refactoring later)
```

---

### Edge Cases & Error States Round (Phase 3e)

**Purpose:** Systematically identify unhappy paths BEFORE scoping stories. Evidence directionally supports that earlier defect discovery reduces cost significantly (Boehm 1981, NIST 2002).

```markdown
Before we scope the stories, let's identify which edge cases and error states matter for {feature}.

These 10 categories are the most commonly missed:

| # | Category | Example for {feature} | Relevant? |
|---|----------|----------------------|-----------|
| 1 | **Empty states** — first-time user with no data | {specific example} | Y/N |
| 2 | **Loading states** — what users see during async operations | {specific example} | Y/N |
| 3 | **Error states** — API failures, validation errors, timeouts | {specific example} | Y/N |
| 4 | **Network degradation** — slow connection, offline mode | {specific example} | Y/N |
| 5 | **Permission changes** — access revoked mid-session | {specific example} | Y/N |
| 6 | **Concurrent modifications** — two users editing simultaneously | {specific example} | Y/N |
| 7 | **Boundary values** — min/max inputs, zero items, overflow | {specific example} | Y/N |
| 8 | **Undo/reversal** — can critical actions be reversed? | {specific example} | Y/N |
| 9 | **Interrupted flows** — session timeout, tab close, browser back | {specific example} | Y/N |
| 10 | **External dependencies** — third-party service outages | {specific example} | Y/N |

For each category you mark as relevant, I'll either:
- Create a dedicated error-handling story (for complex cases)
- Add acceptance criteria to existing stories (for simpler cases)

Which categories apply to your feature?
```

**Rules:**
- Always provide feature-specific examples, not generic descriptions
- Mark categories that research identified as high-risk for this domain
- Every story must end up with at least one unhappy-path acceptance criterion

---

### Quality Gates Round (Phase 3f)

```markdown
Final essential question — what quality commands must pass for every story?

1. Build/type checking:
   A. bun run typecheck && bun run lint
   B. bun run check
   C. cargo check && cargo clippy && cargo test
   D. go build ./... && go vet ./...
   E. Other: [specify]

2. Testing:
   A. Run full test suite after each story
   B. Run only affected tests
   C. No automated tests (manual verification)
   D. Other: [specify]

3. For UI stories, include visual verification?
   A. Yes, verify in browser
   B. No, automated tests sufficient
```

---

### Devil's Advocate Round (Phase 3h)

```markdown
Before we finalize the scope, our research flagged these concerns:

1. **{Risk}:** {research finding about why this is dangerous}
   Teams building similar features often struggle with {specific issue}.
   → Are you comfortable with this, or should we add a mitigation story?

2. **{Assumption}:** We're assuming {X}. But research shows {counter-evidence}.
   → Should we validate this assumption before building, or proceed?

3. **{Scope risk}:** Based on the {N} stories planned, this is a {size} effort.
   Research shows success rates drop significantly for PRDs with >20 stories.
   → Should we phase this into multiple releases?

4. **{Edge case coverage}:** Based on your answers, we'll cover {M} of 10 edge case categories.
   The uncovered categories are: {list uncovered ones}.
   → Are you confident these don't apply, or should we reconsider any?

5. **{Missing consideration}:** Research mentioned {thing} that we haven't discussed.
   → Is this relevant to your use case?
```

---

## PRD Self-Validation Checklist (Phase 5c)

The single source of truth for PRD quality gates. Think step-by-step through each item. For each: cite the specific PRD section that satisfies it. If you cannot cite a section, the check FAILS.

```markdown
### Pre-Save Validation

| # | Check | Status |
|---|-------|--------|
| 1 | Problem Statement clearly articulates WHY (not just WHAT) and includes "Why now" | |
| 2 | Every subjective word ("fast", "simple", "intuitive", "easy") replaced with measurable target | |
| 3 | Non-Goals section present with at least 2 explicit exclusions | |
| 4 | Edge Cases & Error States table has at least 2 documented scenarios | |
| 5 | Every user story has at least one unhappy-path acceptance criterion | |
| 6 | Success Metrics table includes baseline (current), target, and timeframe columns | |
| 7 | NFRs all have specific numbers (latency in ms, uptime %, concurrent users, etc.) | |
| 8 | Target Users section includes pain points and current workarounds | |
| 9 | Risks & Mitigations table has probability and impact ratings | |
| 10 | Two engineers reading this independently would build the same thing | |
| 11 | No story exceeds XL (8 story points) — split if needed | |
| 12 | Total stories ≤ 20 (or explicitly phased into multiple releases) | |
| 13 | Assumptions section documents what we believe but haven't validated | |
| 14 | Technical Considerations framed as questions for engineering input, not mandates | |
| 15 | Changelog section present with initial version entry | |

If any check fails → fix before saving.
```

**The simplest quality test:** Would two engineers, reading this PRD independently, build the same thing? If no, the PRD is not ready.

---

## Compressed Research Summary Format

Internal format for passing Phase 2 output to Phase 3. Store after Phase 2c synthesis. Target: < 300 words internal, < 500 words when presented to user.

```markdown
## Research Brief

### Competitors
- {Name}: {approach} — {strength}, {weakness}
- {Name}: {approach} — {strength}, {weakness}

### Best Practices
1. {Practice from authoritative source}
2. {Practice from authoritative source}

### Technical Recommendations
- Stack: {recommended}
- Pattern: {recommended}
- Libraries: {lib1} (v{x}), {lib2} (v{y})

### Risks
- {Risk 1}: {mitigation}
- {Risk 2}: {mitigation}

### User Expectations (minimum)
- {Feature 1}
- {Feature 2}
- {Feature 3}
```
