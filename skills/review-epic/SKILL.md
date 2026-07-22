---
name: review-epic
description: Review and correct one already-implemented PRD epic (`EP-NNN`) as a single unit across its dependency-ordered child stories, with risk-profiled auditing, one consolidated verification pass, and status roll-up. Use when asked to review an epic, validate an implemented EP-NNN, or replace story-by-story review with one epic-level session.
---

# review-epic

Review: $ARGUMENTS

## Objective

Prove that one implemented epic satisfies its outcome, child acceptance criteria, integration contracts, and applicable security boundaries. Correct confirmed defects, run one consolidated verification bundle, then roll up story, epic, and PRD status.

`SCOPE -> AUDIT -> REMEDIATE -> VERIFY -> STATUS`

Print one compact progress line per phase. Maintain a `Proof Ledger` under 300 tokens containing: epic ID, dependency-ordered child stories, review baseline, criterion evidence, review files, context files, risk axes, confirmed findings, commands, status path, and blockers. Update it instead of repeating plans.

The epic is the execution unit. Child stories organize acceptance evidence and dependency reasoning; they never trigger separate review pipelines.

## Profiles

Select the lowest profile covering the highest-risk changed surface. An explicit profile may escalate but never hide a DEEP condition.

| Profile | Use when | Independent audit | Verification budget |
|---|---|---|---|
| FAST | Isolated behavior, clear in-repo pattern, bounded failure impact, no sensitive boundary | None; orchestrator audits the epic diff | At most 2 commands |
| DEFAULT | Shared integration, public behavior, multi-module change, or a familiar new dependency | One correctness review | At most 4 commands |
| DEEP | Auth, payments, PII, destructive data operations, migration, untrusted shell/file/network input, crypto, LLM tools, cross-service behavior, or genuinely unfamiliar logic | One primary risk axis; add a second axis only when correctness and security are distinct high-risk questions | At most 6 commands |

File count alone never selects DEEP.

## Phase 1 - SCOPE

1. Parse the PRD path, `EP-NNN`, optional `--profile fast|default|deep`, and optional `--base <git-ref>`. With no epic ID, select the first epic whose non-cancelled children are all `DONE` or `IN_REVIEW` and whose review is incomplete (`reviewed_at: null` on at least one child). Log the choice.
2. Read the PRD and adjacent status JSON. Extract the epic outcome, definition of done, non-goals, child stories, dependencies, acceptance criteria, explicit quality gates, and files excluded by the PRD.
3. Include every non-cancelled child story, including stories already marked `DONE`, and topologically order them for evidence mapping. If the tracker contains `TODO`, `IN_PROGRESS`, or `BLOCKED` children, inspect only enough local evidence to distinguish stale status from incomplete implementation. Correct stale status in Phase 5; stop with the incomplete children when the epic is genuinely unfinished.
4. Capture branch, HEAD, worktree state, and review baseline. Prefer explicit `--base`; otherwise use the current branch merge-base with `main` or `master`, then include staged, unstaged, and relevant untracked files.
5. Build the epic file set by mapping each candidate file to at least one child criterion, the epic outcome, or a shared integration point. Preserve unrelated user changes. Treat direct callers and imports as read-only context. Exclude lock files, generated code, vendor/build output, and binary assets unless the epic explicitly changes them. Keep a changed manifest in scope.
6. If several epics share the branch and local evidence cannot isolate this epic, stop with the exact ambiguous files and baseline instead of reviewing unrelated work.
7. Select the profile from the changed attack surface and integration risk.
8. Build the Proof Ledger and a one-screen review card: epic outcome, ordered stories, criterion-to-proof plan, review files, context files, baseline, profile, risk axes, and final gate bundle.

Use an evidence helper only for one named uncertainty that local code, PRD, and history cannot resolve. FAST uses no helper. DEFAULT uses at most one. DEEP uses at most two for distinct gaps. Read [Phase Protocols](references/phase-protocols.md) only when a helper or independent audit is required.

**Gate:** One epic, its ordered child stories, exact diff, profile, proof plan, and verification bundle are known.

## Phase 2 - AUDIT

1. Read the complete epic diff once from the Phase 1 baseline. Read full files only where the diff lacks enough context.
2. Map every child acceptance criterion to `PASS`, `PARTIAL`, `FAIL`, or `MANUAL`, with `file:line`, test, command, or explicit missing evidence. Check the epic definition of done after all child mappings.
3. Review cross-story integration in dependency order. Focus on contract mismatches between slices, invalid state transitions, missing unhappy paths, regressions at shared integration points, and criteria implemented in isolation but broken in composition.
4. Audit applicable correctness, error handling, test, performance, and security boundaries. Inspect secrets and dependencies only when config or manifests changed. Use current documentation or advisories only to resolve a version-sensitive claim that materially affects the verdict.
5. Apply the profile:
   - FAST: complete the orchestrator audit without a helper.
   - DEFAULT: launch one fresh-context correctness review over the epic diff and one-hop context.
   - DEEP: launch one correctness or security audit for the primary risk axis. Launch both in parallel only when they answer distinct high-risk questions.
6. Verify every proposed finding directly against the cited code and surrounding context. Keep only `CONFIRMED` findings. Classify them as mandatory (`CRITICAL`, `HIGH`, `MUST_FIX`), scoped improvement (`MEDIUM`, `SHOULD_FIX`), or tracked outside the epic.

Cap independent output at four high-value findings per file and 800 tokens per axis. Require `file:line`, affected criterion or epic outcome, concrete failure path, and smallest viable correction. Style preferences do not enter the finding set.

**Gate:** Every criterion is accounted for, the epic definition of done is assessed, and every retained finding is confirmed by local evidence.

## Phase 3 - REMEDIATE

1. Consolidate confirmed findings and failed criteria by affected story and shared integration point.
2. Apply one coherent remediation pass for all `CRITICAL`, `HIGH`, and `MUST_FIX` findings. Apply `MEDIUM` or `SHOULD_FIX` only when the change is directly in scope, low-risk, and has a clear proof oracle.
3. Preserve the epic boundary and unrelated work. When a correction requires an irreversible product decision, destructive migration, or architecture expansion absent from the PRD, leave the affected story unresolved and record the decision blocker.
4. Inspect every patched line and its immediate callers. Add only directly affected proof to the final gate bundle. Avoid re-running the full audit.
5. If a mandatory finding survives the pass, stop the correction loop and carry it to STATUS with the attempted fix and exact blocker.

**Gate:** No confirmed mandatory finding remains, or every survivor has a concrete blocker and affected story.

## Phase 4 - VERIFY

Run one consolidated gate bundle after remediation. Use the smallest complete bundle in this order:

1. Explicit PRD quality gates applicable to the epic surface.
2. Acceptance tests proving changed behavior and cross-story integration.
3. One configured type, build, lint, or format command only when it proves something not already covered.
4. A full suite only when the PRD requires it, the epic changes a shared/public contract, or DEEP regression scope cannot be bounded.
5. A supply-chain check only when a manifest changed and the project already configures it.

Prefer aggregate project scripts and skip commands they subsume. Respect the profile budget unless explicit non-overlapping PRD gates exceed it; then run the required gates and record why.

When a command fails, correct the specific cause and rerun only that command plus directly affected acceptance tests. Rerun the full bundle only when the correction changes a shared contract or test infrastructure. After the same cause fails twice, change approach once and stop if unresolved. Record exact commands and results. Mark manual criteria as manual.

**Gate:** Every criterion is proven, manual, or unresolved; every required command has an honest result.

## Phase 5 - STATUS

Reuse the Proof Ledger instead of rereading the epic or rerunning checks.

1. Set `reviewed_at` to the current date for every reviewed, non-cancelled child story.
2. Set a story to `DONE` only when all its criteria are proven, required gates pass, and no mandatory finding or manual verification remains. Set `IN_REVIEW` when implementation exists but required manual proof or an actionable correction remains. Use `BLOCKED` when an external dependency, missing irreversible decision, or repeated technical failure prevents further work. A previously `DONE` story may return to `IN_REVIEW` or `BLOCKED` when the audit disproves completion.
3. Set `completed_at` for newly completed stories and preserve existing completion dates when the verdict remains valid.
4. Recalculate `stories_done`, epic status, and PRD status. Set the epic to `DONE` only when all children are `DONE` or `CANCELLED`.
5. Save the adjacent status JSON. Produce one compact receipt containing: epic and profile, per-story verdicts, epic definition-of-done verdict, confirmed and fixed findings, acceptance evidence, commands and results, status changes, manual proof, and blockers.

**Gate:** The status tracker and receipt describe the same evidence and unresolved work.

## Error Handling

| Error | Action |
|---|---|
| PRD not found | Infer from the current directory or fail with the attempted paths |
| Explicit epic ID not found | Fail with the requested ID and available epic IDs |
| No eligible unreviewed epic | Report the status evidence and stop |
| Epic already reviewed | Report the existing evidence and stop unless an explicit epic ID requests re-review |
| No reviewable diff | Report the baseline and mapping evidence; do not infer implementation from status alone |
| Ambiguous cross-epic diff | Report ambiguous files and require a narrower base or isolated branch |
| Evidence helper fails | Continue from sufficient local evidence or mark the affected claim unverified |
| Verification fails twice for the same cause | Change approach once, then stop and mark affected stories `BLOCKED` |

## Constraints

- **Always:** Preserve the epic boundary, review child stories in dependency order, map every criterion to evidence, validate helper findings locally, remediate before one consolidated verification pass, and roll up story, epic, and PRD status honestly.
- **Never:** Run a complete pipeline per story, review unrelated branch changes, treat status as implementation proof, launch helpers for FAST, launch generic research, install tools, invent acceptance evidence, mark manual proof as automated, commit, or push.

## Examples

- `/review-epic tasks/prd-notifications.md EP-002`
- `/review-epic tasks/prd-billing.md EP-003 --profile deep`
- `/review-epic tasks/prd-search.md EP-001 --base feature/search-start`
