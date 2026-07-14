# Remediation Protocol — Phase 5

Fix validated issues from Phases 3-4, using risk-tiered autonomy with scope discipline.

## 5a — Scope-Guard Check (before fixing)

Count the remediation plan scope: number of distinct fix operations, files affected, estimated LOC delta. Apply [Scope Guard](~/.claude/skills/_shared/scope-guard.md) thresholds:
- Fix operations > 7 OR files to modify > 20 OR estimated LOC delta > 800 → trigger SCOPE ALERT
- If triggered: log the scope warning banner, then proceed (the user invoked review explicitly)
- If not triggered: proceed normally

## 5b — Consolidate and Triage Findings

Merge validated findings from Phase 4.5 (only CONFIRMED/PARTIAL with HIGH/MEDIUM confidence), plus Phase 2.5 static analysis results, into a single prioritized list:

| Priority | Source | Action |
|----------|--------|--------|
| CRITICAL (security) | Phase 4 | Fix immediately |
| HIGH (security) | Phase 4 | Fix immediately |
| MUST_FIX (review) | Phase 3 | Fix immediately |
| Static analysis errors | Phase 2.5 | Fix immediately |
| MEDIUM (security) | Phase 4 | Fix if mechanical oracle exists (lint rule, type error, grep pattern) |
| Static analysis warnings | Phase 2.5 | Fix if mechanical oracle exists |
| SHOULD_FIX (review) | Phase 3 | **Report only — do NOT auto-fix.** Listed in "Observations" section of report. Semantic findings (naming, readability, architecture style) have no binary resolution test and cause fix oscillation if entered into the loop. |
| LOW/INFO/CONSIDER | Phase 3+4 | Report only, skip fixing |

Display the consolidated list to the user before fixing.

## 5c — Risk-Tiered Autonomy

Classify each fix before applying:

| Risk Level | Criteria | Action |
|------------|----------|--------|
| **LOW risk** | Unused imports, formatting, lint fixes, dead code removal, typos | Auto-fix without confirmation |
| **MEDIUM risk** | Logic changes, error handling improvements, missing validation, performance fixes | Fix and show the diff — proceed unless user objects |
| **HIGH risk** | Auth/authorization changes, data access patterns, cryptography, billing logic, API contracts | Apply with extra caution, log detailed rationale |

## 5d — Fix Loop (max 3 iterations)

**Iteration 1:** Fix all CRITICAL + HIGH + MUST_FIX + static errors:
1. Classify each fix by risk level (5c)
2. For LOW risk: apply silently
3. For MEDIUM risk: apply and show diff
4. For HIGH risk: present fix, await confirmation
5. After each fix: run the specific test/check that validates it
6. After all fixes: re-run quality gates + static analysis

**Iteration 2 (if needed):** Fix remaining MEDIUM (security) + static warnings that have a **mechanical resolution oracle** (lint rule, type error, grep-verifiable pattern):
1. Same protocol: classify, fix, test, verify
2. Re-run quality gates
3. **SHOULD_FIX items are NOT included in iteration 2.** They are semantic findings (naming, readability, architecture style) with no binary test to confirm resolution. Including them causes fix oscillation. They appear in the report's "Observations" section only.

**Iteration 3 (if needed):** Address remaining issues or re-fix regressions:
1. If new issues were introduced by fixes → fix them
2. If original issues persist → escalate to user
3. If the fix oscillates (fix A breaks B, fix B breaks A) → stop and escalate

**After each fix:**
1. Re-run the specific check that caught the issue
2. Run a **regression check**: re-scan the 3 files closest to the fix for new issues introduced by the change
3. If new issues appear from the fix itself: mark as "introduced by fix" and prioritize in next iteration

**After each iteration:**
- Run quality gates (PRD-specified + language-specific)
- Run tests to verify no regressions
- Re-run static analysis to verify no new warnings introduced
- Check that previous fixes still hold

## 5e — Scope Creep Guard

If any fix modifies files not in the original findings list, log a warning but proceed only if the fix is directly required. Remediation without scope control cascades into unrelated refactors.

## 5f — Fresh-Context Exit Verification

After all fix iterations, spawn a lightweight `explorer` subagent scoped only to files modified during remediation. Verify each **MUST_FIX/CRITICAL/HIGH** fix actually resolves its flagged issue and no new CRITICAL/HIGH issues were introduced. **Do NOT re-evaluate SHOULD_FIX items** — they have no mechanical oracle and re-evaluation produces non-deterministic results. This prevents self-validation bias (Actor/Critic isolation extends to remediation verification).

## 5g — Escalation (after 3 iterations)

Stop and present remaining issues to the user:
- What was tried
- Why it didn't resolve
- Whether the issue oscillates (two fixes conflict)
- Recommended manual action
