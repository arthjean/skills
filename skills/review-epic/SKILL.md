---
name: review-epic
description: Certify one implemented PRD epic (EP-NNN) as a single unit and write its final status. Use when asked to review, certify, validate, or recheck an epic.
argument-hint: "[prd-path] [EP-NNN] [--profile fast|default|deep] [--base <git-ref>] [--quality]"
---

# review-epic

Review: $ARGUMENTS

Prove the epic against its outcome, child criteria, integration contracts, and security boundaries. Fix confirmed defects, then certify only the final repository state. This skill alone writes `DONE`, `BLOCKED`, or a downgrade. Explicit user instructions override anything below.

## Scope

Resolve the PRD path, epic ID, and optional flags. Without an epic ID, take the first epic at `IN_REVIEW`, else the first whose non-cancelled children are implemented but uncertified. Include every non-cancelled child, `IN_REVIEW` and `DONE` ones too, in dependency order. Tracker status is never proof: `IN_REVIEW` is the expected inbound state and carries none.

Isolate the epic diff. Prefer `--base`; otherwise use the default-branch merge-base only when it isolates the epic unambiguously. Include committed, staged, unstaged, and untracked changes. Review lockfiles, generated files, and binaries through their source or metadata rather than excluding them. When several epics cannot be separated, stop and report the ambiguous files and baseline.

External evidence is worth fetching only for one named current or version-sensitive claim that can change a verdict, finding, or gate. One route per question; record fact, source, and review impact.

## Reachability first

A diff shows what was added. Missing wiring is an absence and never appears in it, so a diff-only audit is blind to the most common class of "complete but inert" epics. Read the epic in reverse, from the repository's real execution roots (route table, command registry, rendered component tree, dependency container, migration list, scheduler, event subscriptions, config readers, public package exports) to the new code, against the final repository state:

- Every symbol the epic adds or modifies has a referent outside its own module and outside tests. Test-only referents are `BLOCKING` unless the PRD names a public API or extension point.
- Every criterion traces from an execution root to the implementing code.
- A replaced path is no longer reachable. Both paths wired means runtime behavior did not change.
- Every added config key, environment variable, or flag is read, with its default in the manifest the project really loads.
- A static reference proves linkage, not reachability. Conditional entry (route never registered, dead branch, component never rendered, flag never enabled) needs an execution observation: a test through the entry point, a log, a trace, or a manual check.

Then review cross-story composition in dependency order: shared contracts, state transitions, unhappy paths, regressions, criteria that pass alone and fail together. Audit correctness, error handling, tests, performance cliffs, dependencies, and security boundaries where applicable. Style and unsupported speculation are out of scope.

## Verdicts and findings

Per criterion: `PASS` (current proof reached from a real entry point), `FAIL` (contradicted, unreachable, or missing), `MANUAL_PROVEN` (observed manual check), `MANUAL_PENDING` (required observation unavailable).

Per finding: `BLOCKING` (violates a criterion, definition of done, correctness or security invariant, leaves behavior unreachable, or regresses), `NON_BLOCKING` (confirmed in-scope improvement, where all maintainability findings land), `OUTSIDE_EPIC`. Each finding carries `file:line`, the affected criterion, the concrete failure path, and the smallest complete correction, deduplicated by root cause.

Independent review is optional and capped: for the DEEP tier, at most one fresh read-only reviewer on `correctness` or `security`; with `--quality`, at most one `maintainability` reviewer scoped to the epic diff, briefed with [the maintainability baseline](references/maintainability-baseline.md). Verify every returned finding against local code before keeping it.

## Remediate

One coherent repair pass for `BLOCKING` findings, wiring defects first. Apply a `NON_BLOCKING` fix only when it is in scope, low-risk, and has a clear proof oracle; a maintainability fix only inside the epic diff. Add regression tests for mechanically testable defects, entering through the real path. Wire unreachable behavior yourself when the entry point is unambiguous; otherwise mark the criterion `FAIL` and report the gap. Leave a criterion unresolved rather than take an irreversible decision, destructive migration, or architecture expansion. If the same cause survives two distinct repair approaches, record the blocker and stop repairing that path. Recheck repaired paths; do not rerun the whole audit.

## Validate and certify

Validate once after the last change, in proportion to the changed surface: acceptance and regression tests for every child, integration tests for shared contracts, configured checks for changed files, the relevant full suite for DEEP-tier surfaces (auth, payments, PII, migrations, untrusted input, crypto, destructive operations, LLM tools, cross-service) or when regression scope cannot be bounded, and existing security or supply-chain checks when manifests or trust boundaries changed. Local checks are authorized without asking. Only the passing run after the final change certifies.

A story is `DONE` only when every criterion is `PASS` or `MANUAL_PROVEN`, required gates pass, and no blocking finding remains. `IN_REVIEW` covers pending manual proof or actionable correction; `BLOCKED` covers external dependencies, missing irreversible decisions, or repeated technical failure. Downgrade a `DONE` story when current evidence disproves it, preserving a valid `completed_at`. Set `reviewed_at` and `completed_at` per repository convention, recompute counters, and set the epic `DONE` only when every child is `DONE` or `CANCELLED`.

Receipt: epic and tier, per-story and epic verdicts, criterion evidence with entry points, wiring defects, fixed findings, unapplied maintainability findings, commands and results, status changes, manual proof, blockers.

## Boundaries

Preserve the epic boundary and unrelated worktree changes. Never weaken tests, fabricate findings, or mark pending manual proof complete. Maintainability never gates certification. Commit, push, publish, destructive external actions, and scope expansion need explicit authorization.

## Examples

- `/review-epic tasks/prd-notifications.md EP-002`
- `/review-epic tasks/prd-search.md EP-001 --base feature/search-start --quality`
