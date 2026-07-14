---
name: implement-epic
description: "Implement one complete PRD epic (`EP-NNN`) through dependency-ordered story slices, one consolidated validation pass, risk-triggered review, and status roll-up. Use when asked to implement an epic, execute an EP-NNN from a PRD, or progress a PRD epic end to end without story-by-story session overhead."
---

# implement-epic

Implement: $ARGUMENTS

## Objective

Complete one epic with the smallest evidence, review, and verification loop that can prove it works.

`SCOPE -> IMPLEMENT -> REVIEW -> VERIFY -> STATUS`

Print one compact progress line per phase. Keep a `Proof Ledger` under 250 tokens containing: epic ID, ordered incomplete stories, acceptance proof, changed files, risk flags, commands run, status path, and unresolved blockers. Update it instead of repeating plans.

## Profiles

Select the lowest profile that covers the highest-risk incomplete story. An explicit profile may escalate but never hide a DEEP condition.

| Profile | Use when | Review | Verification budget |
|---|---|---|---|
| FAST | Clear in-repo pattern, isolated behavior, no sensitive boundary | Diff self-check only | At most 2 commands |
| DEFAULT | Shared integration, public behavior, multi-module change, or new dependency using familiar patterns | One orchestrator review | At most 4 commands |
| DEEP | Auth, payments, PII, destructive data operations, migration, untrusted shell/file/network input, crypto, LLM tools, cross-service behavior, or genuinely unfamiliar logic | One orchestrator review plus at most one independent review axis; add a second axis only for a distinct security surface | At most 6 commands |

Count an aggregate project script as one command even when it runs several tools. File count alone does not select DEEP.

## Phase 1 - SCOPE

1. Parse the PRD path, epic ID, and optional `--profile fast|default|deep`. If no epic is provided, select the first epic with incomplete stories and log the choice.
2. Read the PRD and adjacent status JSON. Extract the epic outcome, non-goals, child stories, dependencies, acceptance criteria, and explicit quality gates.
3. Exclude stories already `DONE` or `CANCELLED`. Topologically order the remaining stories and stop on an incomplete external dependency.
4. Capture the Git baseline and dirty worktree. Preserve unrelated user changes.
5. Inspect the shared integration points and 3-8 relevant files. Read more only when the next implementation decision is ambiguous.
6. Build the Proof Ledger and a one-screen execution card: slices, files with purpose, acceptance proof, integration points, risk, and final gate bundle.

Use a research helper only for one named blocker that local evidence cannot resolve:
- FAST: no helper. Escalate to DEFAULT if required.
- DEFAULT: at most one evidence helper.
- DEEP: at most two evidence helpers for distinct gaps, launched in parallel when independent.

Do not scan memory directories, browse, fetch documentation, or inventory the repository by default. The runtime may supply relevant memory independently. Read [Phase Protocols](references/phase-protocols.md) only when a helper or independent review is actually required.

If the PRD omits an irreversible product or safety decision, mark the affected story `BLOCKED`, save the tracker, and stop.

**Gate:** Ordered slices, proof ledger, profile, baseline, and final gate bundle are known.

## Phase 2 - IMPLEMENT

Implement incomplete stories in dependency order. Read each file before editing it. Set a story to `IN_PROGRESS` when its slice begins and keep the epic and PRD `IN_PROGRESS`.

Do not run a check after every story. Run a focused check during implementation only when at least one condition applies:
- A later slice depends on the changed contract.
- The check resolves an active ambiguity or catches a likely regression before more code depends on it.
- The new focused test is cheap and directly proves the slice.

Batch adjacent slices that share the same integration point. Respect these in-progress check caps: FAST 0, DEFAULT 1, DEEP 2. These caps are separate from the final verification budget.

Update the Proof Ledger after each slice with changed files and intended acceptance proof. Do not mark a story `DONE` yet. If a dependency breaks or an irreversible ambiguity appears, mark the affected slice `BLOCKED`, save the tracker, and stop before dependent slices.

**Gate:** Eligible slices are implemented and every criterion has a concrete proof path.

## Phase 3 - REVIEW

Review before final validation so fixes are included in the single gate run.

1. Read the epic diff once from the Phase 1 baseline.
2. Check the epic definition of done, child acceptance criteria, relevant unhappy paths, local conventions, and only applicable security boundaries.
3. FAST: fix obvious issues inline and do not launch a reviewer.
4. DEFAULT: perform one orchestrator review. Do not launch an independent reviewer.
5. DEEP: launch one independent code review only for substantial business logic or a public contract. Use a security audit instead when the sensitive boundary is the primary risk. Launch both in parallel only when they cover distinct high-risk questions.
6. Fix CRITICAL, HIGH, and MUST_FIX findings in one remediation pass. Apply MEDIUM or SHOULD_FIX only when directly in scope. Do not re-review the full diff after remediation; inspect the patched lines and add their proof to the final gate bundle.

If a mandatory finding remains after one remediation pass, stop and report it rather than entering another review loop.

**Gate:** No unresolved mandatory review finding remains.

## Phase 4 - VERIFY

Run one consolidated gate bundle after all implementation and review fixes. Do not install tools and do not add a full repository suite merely because it exists.

Build the smallest complete bundle in this order:
1. Explicit PRD quality gates that apply to the changed surface.
2. Acceptance tests targeting changed behavior.
3. One configured type, build, lint, or format command only when it proves something not already covered.
4. Full suite only when the PRD explicitly requires it, the epic changes a shared or public contract, or the selected profile is DEEP and regression scope cannot be bounded.
5. Supply-chain check only when a manifest changed and the project already configures the check.

Prefer aggregate project scripts and skip commands they subsume. Respect the profile verification budget. If explicit non-overlapping PRD gates exceed the budget, run them and report the reason.

When a command fails, fix the specific failure and rerun only that command plus any directly affected acceptance test. Do not rerun the whole bundle unless the fix changes a shared contract or test infrastructure.

Record exact commands and results in the Proof Ledger. Mark manual criteria explicitly instead of inventing automated proof.

**Gate:** Required commands pass and every acceptance criterion is proven, manual, or blocked.

## Phase 5 - STATUS

Reuse the Proof Ledger. Do not reread every story or rerun checks.

1. Set a child story to `DONE` when every criterion is proven and no manual verification remains.
2. Use `IN_REVIEW` when only manual verification remains and `BLOCKED` when work cannot continue.
3. Set `completed_at` for newly completed stories.
4. Roll up `stories_done`, epic status, and PRD status. Set the epic to `DONE` only when all children are `DONE` or `CANCELLED`.
5. Save the status JSON.

Return a compact receipt: epic and profile, completed and pending stories, acceptance proof, commands run, review mode, status changes, and remaining manual or blocked work. Report token use only from host telemetry.

## Error Handling

| Error | Action |
|---|---|
| PRD not found | Infer from the current directory or fail clearly |
| Epic not found | Select the first epic with incomplete stories and log it |
| Incomplete dependency | Mark the dependent story `BLOCKED`, save, and stop |
| Evidence helper fails | Continue from sufficient local evidence or block the affected slice |
| Verification fails twice for the same cause | Stop repeating the command, change approach once, then block if unresolved |
| Epic already done | Log the state and stop unless the user explicitly requests reimplementation |

## Constraints

- **Always:** Preserve the epic boundary, map acceptance criteria to proof, review the final diff once, run one consolidated verification bundle, and update story, epic, and PRD status honestly.
- **Never:** Test after every story by default, launch review helpers for FAST or DEFAULT, run both targeted and full suites without a distinct reason, rerun all gates after every fix, inventory the whole repository, invent criteria, or mark manual proof as automated.

## Examples

- `/implement-epic tasks/prd-notifications.md EP-002`
- `/implement-epic tasks/prd-billing.md EP-003 --profile deep`
- `/implement-epic tasks/prd-search.md EP-001`
