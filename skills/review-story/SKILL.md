---
model: opus
name: review-story
description: "End-to-end review and correction workflow for an ALREADY-IMPLEMENTED user story (or full PRD whose stories have been coded). Reviews the CODE against its PRD — not the PRD document. Orchestrates 7 phases: intake, research (/meta-code pipeline), static analysis, parallel AI code review + security audit (3-layer: SAST/Secrets/SCA), risk-tiered remediation, and executive summary report. Signal-over-noise design: pre-filters generated files, scopes to diff+1 hop, targets 2-4 high-value findings per file. Do NOT use for greenfield scaffolding, code generation, or for reviewing an unimplemented PRD document (use /meta-review-prd for that). Does not commit or push. Invoke with /review-story [prd-path] [story-id?]."
argument-hint: "[prd-path] [story-id?]"
---

# review-story — PRD Review & Correction Pipeline

Review the following: $ARGUMENTS

## Overview

Review and correction pipeline for already-implemented user stories. Takes a PRD (single story or full PRD), researches best practices (meta-code-style pattern), runs static analysis, then parallel AI code review + security audit, validates findings, then risk-tiered remediation with scope-guard. Stops after correction — no commit or push.

**Key principles:**
- Context compression at every phase boundary (Anthropic context engineering)
- Research-informed review — understand best practices before judging code
- Intent-aware — understand *why* the code was written before judging *how*
- Static-first — run deterministic checks (cheap, fast, high-precision) before AI review
- Fresh-context reviewers — subagents with no bias toward the code (Actor/Critic isolation)
- Signal over noise — target 2-4 high-value findings per file, suppress style opinions
- Scope discipline — review diff + 1 dependency hop, not the entire codebase
- Risk-tiered fixes — auto-fix low-risk issues, confirm medium-risk, escalate high-risk
- No commit — the user controls when and how to commit

## Execution Flow

`INTAKE → RESEARCH → STATIC → REVIEW+SECURITY (parallel) → VALIDATE → REMEDIATE → SUMMARY`

Print `[Phase N] PHASE_NAME` before each phase.

## Phase-by-Phase Execution

### Phase 1 — INTAKE

Parse the PRD and identify the review scope.

**1a. Parse arguments:**

- `$ARGUMENTS` contains a file path → read that file as the PRD
- `$ARGUMENTS` contains a story ID (e.g., `US-001`) → review only that story
- No story ID → review ALL stories in the PRD (full PRD review mode)
- If arguments are ambiguous → infer the most likely match from PRD content

**1b. Read and parse the PRD:**

Read the PRD file. For each story in scope, extract:
- **Story title and description**
- **Acceptance criteria** (checklist items)
- **Quality gates** (from the PRD's Quality Gates section, if present)
- **Functional requirements** (from the PRD, if present)

**1c. Map changed files:**

Identify which files implement the stories. Run in order, stop at first success:

```bash
git diff --name-only --stat main...HEAD                                    # 1. Branch diff vs main (preferred)
git diff --name-only --stat master...HEAD                                  # 2. Fallback: vs master
git diff --name-only --stat HEAD && git diff --name-only --stat --cached   # 3. Fallback: unstaged + staged
# 4. Fallback: ask the user
```

If reviewing a single story and the file list is large (>10 files), infer the most relevant files from imports and naming. Otherwise review all.

If reviewing a full PRD, review all changed files.

**1d. Pre-filter and size assessment:**

**Pre-filter** — remove non-reviewable files from scope:
- Lock files: `package-lock.json`, `yarn.lock`, `Cargo.lock`, `pnpm-lock.yaml`, `Gemfile.lock`, `poetry.lock`
- Generated files: `*.generated.*`, `*.g.dart`, `*.pb.go`, files with `@generated` header
- Vendor directories: `vendor/`, `node_modules/`, `third_party/`
- Build output: `dist/`, `build/`, `target/`, `.next/`
- Assets: binary files, images, fonts

**Size assessment** — compute total lines changed (additions + modifications):
- **< 400 lines:** Optimal — proceed normally
- **400-500 lines:** Acceptable — proceed with note
- **> 500 lines:** Log a chunking warning, then proceed — review by story or logical grouping if multiple stories are in scope.

**1e. Read all files in scope:**

Read every file identified in 1c-1d using the Read tool. Also read **1 dependency hop**: for each changed file, identify its direct importers/callers and the files it imports. This provides context without reviewing the entire codebase.

**1f. Extract developer intent:**

Understand *why* the code was written this way before judging it. This reduces false positives from misunderstood context.

```bash
git log --oneline main...HEAD    # Sequence of changes
git log --format="%B" main...HEAD  # Full commit messages with context
```

Extract and compress into a brief intent summary: what the developer was trying to accomplish, any noted constraints or tradeoffs, and any "TODO" or "HACK" markers that signal known technical debt.

Pass this intent context to the review agents alongside the research brief.

**1g. Display review scope:**

```
## Review Scope

**Mode:** {Single Story: US-XXX | Full PRD}
**Stories in scope:** {count}
**Files to review:** {count} ({total_lines_changed} lines changed)
**Files filtered out:** {count} (lock files, generated, vendor)
**Size assessment:** {OPTIMAL | ACCEPTABLE | LARGE — consider chunking}

### Stories
- US-001: {title} — {status: found/not-found in changed files}
- US-002: {title} — ...

### Files
- path/to/file.ext (+{added} -{removed})
- ...

### Context Files (1-hop dependencies, read-only context)
- path/to/caller.ext (imports changed file)
- ...

```

**1h. Check for REVIEW.md:**

Look for `REVIEW.md` files in the repo hierarchy (project root, directories containing changed files). REVIEW.md contains review-specific instructions separate from CLAUDE.md — conventions the review agents should enforce. If found, pass its contents to Phase 3 and Phase 4 agents alongside the research brief.

**1i. Detect status tracking:**

Look for a status JSON file alongside the PRD:
- If PRD is `tasks/prd-foo.md` → check for `tasks/prd-foo-status.json`
- If found → read it, verify the target story is `IN_REVIEW`
- If not found → create a minimal status JSON from the PRD's story list
- If story status is not `IN_REVIEW` → log a warning and proceed (the user invoked the command explicitly)

**GATE:** Scope parsed and displayed.

**Phase 1 Summary:** Stories in scope (IDs + titles + criteria), filtered file list with line counts, context files (1-hop), intent summary, size assessment, status JSON path.

---

### Phase 2 — RESEARCH (meta-code-style research pattern)

Research best practices before judging code. See [Review Protocols](references/review-protocols.md) for exact prompt templates.

**2a.** Spawn `web-researcher` to research current best practices, anti-patterns, and security considerations for the feature area. Wait for completion and compress to fewer than 500 words.

**2b.** Detect codebase manifests (Cargo.toml, package.json, pyproject.toml, go.mod).

**2c.** Spawn `explorer` and `docs-researcher` in parallel when both routes are needed. Explore codebase patterns and fetch version-sensitive library documentation via ctx7 CLI.

**2d.** Synthesize all findings into a compressed review brief (<500 words): best practices, pitfalls, correct API usage, security considerations. This brief is passed to Phase 3 and 4 agents.

**GATE:** Research synthesis complete.

**Phase 2 Summary:** Compressed review brief (<500 words) — best practices, anti-patterns, correct API usage, security considerations.

---

### Phase 2.5 — STATIC ANALYSIS (deterministic, before AI review)

Run deterministic checks (lint, format, type-check) before spending AI tokens. See [Static Analysis Protocol](references/static-analysis-protocol.md) for language-specific commands, tables, and detailed steps.

**Key steps:** Detect project tools (2.5a), run type checking (2.5b), collect results mapped to severities (2.5c), git blame hotspot analysis (2.5d), generate 3-5 targeted risk hypotheses (2.5e). Only run tools already configured — do NOT install new ones.

**GATE:** Static analysis complete. Results stored for Phase 5.

**Phase 2.5 Summary:** Static analysis findings (errors/warnings mapped to severities), risk hypotheses list.

---

### Phase 3 — CODE REVIEW (parallel with Phase 4)

Spawn explorer with the Code Review template from review-protocols.md. The review agent checks **8 categories** in priority order: (1) Acceptance Criteria Compliance, (2) Correctness, (3) Architecture & Design, (4) Error Handling & Logging, (5) Quality, (6) Performance, (7) Tests, (8) Best Practices from Phase 2 research.

See [Review Protocols](references/review-protocols.md) — Phase 3 prompt template for the full checklist with detailed sub-items per category.

**Scope discipline:** Review the diff + 1 dependency hop only. If a finding concerns code outside this scope, classify it as "TRACKED — out of scope" rather than a blocking finding.

**Signal-to-noise control:** Target 2-4 high-value findings per file. Suppress style opinions and micro-optimizations. Every finding must answer: "Does this make the codebase health better or worse?" If not — omit it.

**Finding tiers:**
- **Tier 1 (always report):** Runtime errors, crashes, exploitable vulnerabilities, data loss risks
- **Tier 2 (report when impactful):** Architectural inconsistencies, measurable performance issues, missing error handling
- **Tier 3 (suppress unless egregious):** Style preferences, micro-optimizations, subjective naming

Output: Structured report with MUST_FIX / SHOULD_FIX / CONSIDER / OK findings. Each finding includes an **Impact** statement explaining WHY it matters.

**GATE:** (shared with Phase 4 — both must complete before proceeding)

---

### Phase 4 — SECURITY REVIEW (parallel with Phase 3)

Spawn explorer with the Security template from review-protocols.md. The security agent follows the `/security-review` protocol with extended coverage:

**Layer 1 — SAST (Source Analysis):**
1. Read all changed files
2. Audit against OWASP Top 10 2025 (includes LLM-specific threats + supply chain risks)
3. Check for injection, auth issues, insecure crypto, data handling
4. AI-generated code anti-patterns (eval, innerHTML, .unwrap() on user input, shell=True)

**Layer 2 — Secrets Detection:**
5. Scan for hardcoded passwords, API keys, tokens, connection strings
6. Check for secrets in config files that will be committed
7. Verify `.gitignore` covers sensitive files (`.env`, credentials, key files)

**Layer 3 — Dependency Scanning (SCA):**
8. Check `Cargo.toml` / `package.json` / `pyproject.toml` for known vulnerable dependencies
9. Flag dependencies with no maintenance (archived repos, no updates in 2+ years)
10. Check for typosquatting risks on new dependencies

**Blocking strategy (tiered):**
- CRITICAL/HIGH: Block — these represent exploitable vulnerabilities
- MEDIUM: Report — create ticket, don't block review
- LOW/INFO: Informational tracking only

Output: Structured security report with CRITICAL/HIGH/MEDIUM/LOW/INFO findings. Each finding includes CWE reference and before/after code remediation.

**Spawn Phase 3 and Phase 4 in a SINGLE message** for true parallel execution.

**GATE:** Both reviews complete.

**Phase 3+4 Summary:** Consolidated findings (severity + file:line + issue + impact + fix suggestion), review verdict, security verdict.

---

### Phase 4.5 — VALIDATE (verification feedback loop)

Every finding from Phase 3 and 4 is a **claim** about the code. Before spending effort on remediation, verify each claim is real.

**4.5a. For each finding with a file:line citation:**

```
Grep/Read the cited file:line and confirm the described pattern actually exists.
```

- Finding cites `auth.rs:42` with "missing ownership check" → Read auth.rs:42, verify the pattern
- Finding cites `handler.ts:15` with "SQL injection via string concat" → Grep for string concatenation in queries at that location

**4.5b. Classify verification results:**

| Result | Action |
|--------|--------|
| **CONFIRMED** — pattern exists exactly as described | Promote to remediation queue |
| **PARTIAL** — pattern exists but description is inaccurate or exaggerated | Downgrade severity, reclassify, promote with corrected description |
| **REFUTED** — pattern does not exist at cited location | Drop finding, log as false positive |
| **STALE** — file:line has shifted (file was modified) | Re-locate pattern via Grep, update citation, then re-verify |

**4.5c. Numeric confidence scoring (0-100):**

Assign each surviving finding a confidence score on a 0-100 scale (matches Anthropic Code Review plugin pattern):

| Score Range | Label | Criteria |
|-------------|-------|----------|
| **80-100** | HIGH | Static analysis error OR exact Grep match at cited location |
| **50-79** | MEDIUM | Pattern confirmed by reading surrounding context (±10 lines) |
| **25-49** | LOW | Inferred from research, plausible but ambiguous |
| **0-24** | DROP | Not credible — drop the finding |

**Threshold (configurable, default 80):** Only findings scoring >= 80 enter auto-remediation. Findings scoring 50-79 are included in the report but require explicit user opt-in for fixes. Findings below 50 are excluded from remediation entirely.

**4.5d. Report validation results:**

```
## Validation Results
- Findings received: {total from Phase 3 + Phase 4}
- CONFIRMED: {count} (promoted to remediation)
- PARTIAL: {count} (reclassified, promoted)
- REFUTED: {count} (dropped — false positives)
- Signal ratio: {confirmed + partial} / {total} ({percentage}%)
```

**GATE:** Validated finding list ready for remediation.

**Phase 4.5 Summary:** Validated findings only (CONFIRMED/PARTIAL with confidence scores), signal ratio, false positive count.

---

### Phase 5 — REMEDIATE

Fix validated issues from Phases 3-4, using risk-tiered autonomy. See [Remediation Protocol](references/remediation-protocol.md) for full procedure (5a-5g).

**Key rules:**
- **Scope guard** (5a): >7 fix ops OR >20 files OR >800 LOC delta → escalate via [Scope Guard](@~/.claude/skills/_shared/scope-guard.md)
- **Triage** (5b): CRITICAL/HIGH/MUST_FIX → fix immediately. SHOULD_FIX → report only (never enters remediation loop)
- **Risk tiers** (5c): LOW (auto-fix), MEDIUM (fix + show diff), HIGH (await confirmation)
- **Max 3 iterations** (5d): fix → re-test → fix regressions. After 3: escalate to user
- **Scope creep guard** (5e): fix touches files outside scope → ask user first
- **Exit verification** (5f): fresh-context `explorer` verifies MUST_FIX/CRITICAL/HIGH fixes resolved
- **Escalation** (5g): remaining issues → present to user with context

**GATE:** Zero CRITICAL/HIGH/MUST_FIX issues remaining. Quality gates pass. Static analysis clean. SHOULD_FIX items reported as observations (not blocking).

---

### Phase 6 — SUMMARY

Produce the final review report and update status tracking. No commit or push.

**6a. Update PRD status:**

If the review verdict is PASS (zero CRITICAL/HIGH remaining):
1. Set story status: `IN_REVIEW → DONE`
2. Set `reviewed_at` to current date (`YYYY-MM-DD`)
3. Roll-up PRD status: if all stories are `DONE` → PRD status = `DONE`
4. Save the updated status JSON

If the review verdict is FAIL (CRITICAL/HIGH issues remain):
- Do NOT update story status — it stays `IN_REVIEW`
- Note in the summary that the story needs re-implementation

If the status JSON does not exist (edge case), skip silently.

**6b. Produce report:**

## Output Format

See [Output Format Template](references/output-format.md) for the complete Phase 6 report structure.

```
**Verdict:** {ALL_CLEAR | PASS_WITH_FIXES | ISSUES_REMAINING}
**Phase results:** Intake ✓ | Research ✓ | Static ✓ | Review {PASS/FAIL} | Security {PASS/FAIL} | Remediation {PASS/PARTIAL}
```

Key sections: Executive Summary with verdict, Acceptance Criteria table, Validation Results (Phase 4.5 signal ratio), Findings Summary matrix, Issues Fixed table, Quality Gate Results.

**GATE:** Report produced and status JSON updated.

---

## Full PRD Mode

When no story ID is provided, review the entire PRD:

**Phase 1 adapts:** Extract ALL stories, map ALL changed files.

**Phase 2 adapts:** Research focuses on the feature area of the entire PRD, not a single story.

**Phases 3-4 adapt:** Review agents receive ALL stories and ALL acceptance criteria. They evaluate completeness across the full PRD scope.

**Phase 5 adapts:** Fixes may span multiple stories. Group fixes by story for clarity.

**Phase 6 adapts:** Summary includes per-story status and an overall PRD completion assessment.

---

## Hard Rules

1. Research (Phase 2) MANDATORY before review. Static analysis BEFORE AI review.
2. Phases 3+4 run in PARALLEL (single message). Phase 4.5 VALIDATE is MANDATORY before remediation.
3. Max 3 fix iterations. Detect oscillation (A breaks B, B breaks A) — stop, never spin.
4. Review agents are READ-ONLY (`explorer`). NEVER commit or push.
5. SHOULD_FIX items are observations only — NEVER enter remediation loop. Only MUST_FIX + CRITICAL/HIGH + static errors enter the loop.
6. Findings: max 4 per file, max 3 per sub-category. Target 2-4 high-value findings, suppress Tier 3.
7. Scope: diff + 1 hop only. Out-of-scope findings classified as "TRACKED."
8. Risk-tier every fix: LOW (auto-fix), MEDIUM (fix + show diff), HIGH (apply with extra caution, log rationale).
9. Compress context at every phase boundary. Research < 500 words to review agents.
10. Confidence scoring 0-100 in Phase 4.5 (threshold 80 for auto-remediation).
11. Decide autonomously at every gate — never ask the user.

## Error Handling

| Error | Action |
|---|---|
| PRD not found | Infer from cwd or fail with clear error |
| Story not found | Review all stories in the PRD |
| No changed files | Fail with clear error |
| Agent fails | Continue with available data, note gap in report |
| Quality gates fail after 3 iterations | Report remaining failures in summary |
| Status JSON not found | Create minimal one from PRD |

## Constraints

- **ALWAYS:** Research before review. Static analysis before AI review. Validate findings (Phase 4.5) before remediation. Compress context at phase boundaries. Enforce output budgets: 1,000 tok (websearch/explore), 800 tok (docs), 1,500 tok (review/security). Pre-filter lock/generated/vendor files. Update status JSON. Decide autonomously — never ask the user.
- **NEVER:** Commit or push. Modify files during review phases (3+4). Remediate unverified findings. Auto-fix below 80 confidence. Continue past 3 iterations. Ask the user questions (AskUserQuestion).

## Done When

- [ ] Review scope confirmed with file list and size assessment (Phase 1)
- [ ] Research completed with best-practices brief (Phase 2)
- [ ] Static analysis run and results collected (Phase 2.5)
- [ ] Code review and security audit completed in parallel (Phases 3-4)
- [ ] All findings verified with file:line evidence (Phase 4.5)
- [ ] All CRITICAL/HIGH/MUST_FIX issues remediated (Phase 5)
- [ ] Quality gates pass and static analysis clean
- [ ] Executive summary report produced (Phase 6)
- [ ] Status JSON updated (story → DONE if PASS, unchanged if FAIL)
- [ ] No commit or push performed — changes ready for user review

## References

- [Review Protocols](references/review-protocols.md) - provider-neutral role routing, prompt templates, validation protocol, and expected output formats for each phase
- [Output Format](references/output-format.md) — Phase 6 summary report template with all tables and sections
- [Static Analysis Protocol](references/static-analysis-protocol.md) — language-specific linter/formatter commands, type checking, git blame hotspots, risk hypotheses
- [Remediation Protocol](references/remediation-protocol.md) — scope guard, triage table, risk-tiered autonomy, fix loop, exit verification
- [Agent Boundaries](@~/.claude/skills/_shared/agent-boundaries.md) — shared agent delegation rules, call budgets
- [Scope Guard](~/.claude/skills/_shared/scope-guard.md) — threshold definitions and escalation protocol
- [Synthesis Template](~/.claude/skills/_shared/synthesis-template.md) — research synthesis output format
- [Three-Tier Constraints](~/.claude/skills/_shared/three-tier-constraints.md) — ALWAYS/NEVER model
