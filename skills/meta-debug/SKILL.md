---
name: meta-debug
description: "Reproduce-first debugging workflow that diagnoses and fixes errors with targeted local evidence, bounded hypotheses, conditional helpers, two fix attempts maximum, and focused verification. Use when the user provides an error, stack trace, failing command, compiler output, failing test, unexpected behavior, or asks to debug, diagnose, or fix a concrete failure. Do not use for feature implementation, general code review, speculative refactoring, or questions without an observed failure."
---

# meta-debug

Debug the current user-reported failure.

## Objective

Find the root cause, apply the smallest safe fix, and prove it with the original reproducer or the closest deterministic check.

`REPRODUCE/ROUTE -> DIAGNOSE -> FIX -> VERIFY/OUTPUT`

For FAST work, do not print phase headers. For STANDARD and DEEP work, print one compact progress line per phase. Do not narrate skipped helpers or routine reads.

## Profiles

Select the lowest profile that covers the uncertainty and blast radius.

| Profile | Use when | Hypothesis cap | Total helper cap | In-progress checks |
|---|---|---|---|---|
| FAST | Syntax error, missing import, obvious type mismatch, or single failing assertion with a clear local cause | No register | 0 | 0 |
| STANDARD | Clear stack trace, runtime error, config failure, dependency issue, or failing test requiring targeted investigation | 3 | 1 | 0 |
| DEEP | Concurrency, performance regression, missing or misleading stack trace, cross-module state, code generation, architecture-level failure, or genuinely unfamiliar behavior | 4 | 2 | At most 1 discriminating check |

Helper caps cover the entire run. A failed helper still consumes its slot. File count alone does not select DEEP.

## Phase 1 - REPRODUCE/ROUTE

1. Extract the exact error, failing command, relevant file and line, stack frames, version, and environment clues from the user input.
2. Choose the smallest deterministic reproducer: the named failing test, compiler target, command, request, or script.
3. Accept a complete fresh compiler, type-checker, or test output as the failing baseline when rerunning it would add no information. Otherwise run the reproducer once before investigating.
4. Confirm that the failure matches the report. Record only the core error and the command, not the full terminal transcript.
5. Select FAST, STANDARD, or DEEP.

If the failure does not reproduce, do not guess. Check whether it is intermittent or environment-specific, then ask one targeted question for the missing state, input, timing, or command.

Do not run a full suite, broad build, lint, or type-check when a smaller command reproduces the failure.

**Gate:** A failing baseline exists, or the exact missing reproduction input is known.

## Phase 2 - DIAGNOSE

Start with local evidence:

1. Read the failing file and the smallest surrounding function, type, fixture, or configuration needed to understand it.
2. Follow user-code stack frames before framework or generated frames.
3. Search the failing symbol, error code, assertion, or configuration key with targeted `rg`.
4. Inspect the manifest, lockfile, or project configuration only when dependency version or setup is a plausible cause.
5. Prefer observations that eliminate several explanations at once.

FAST uses the visible cause directly and does not create a hypothesis register. STANDARD keeps at most three active hypotheses. DEEP keeps at most four. Maintain hypotheses internally and return the register only when diagnosis remains blocked.

Use a helper only for one named gap local inspection cannot settle. Read [Helper Protocols](references/helper-protocols.md) only when delegation is required.

- Use `agent-explorer` only after 3-5 focused reads leave a cross-module flow, concurrency path, generated boundary, or state interaction unresolved.
- Use `docs-researcher` only for one exact version-sensitive API, configuration, or migration question.
- Use `web-researcher` only when a known external bug, platform issue, compiler issue, or undocumented behavior remains plausible after local and official documentation evidence.

Run independent helpers in parallel when DEEP requires two distinct gaps. Pass only the failing baseline, active hypotheses, relevant versions or paths, and the named question. Do not retry a failed helper through another role.

Declare the root cause clear when direct evidence supports one causal explanation and the proposed fix target. Do not eliminate every theoretical alternative.

### Regression history

Use read-only `git log`, `git show`, `git diff`, or `git blame` first. Do not start `git bisect` automatically.

Use `git bisect` only when all conditions hold:
- The user explicitly approves it.
- The tracked worktree is clean or the work is isolated safely.
- A known good and known bad boundary exist.
- The reproducer is deterministic, reasonably fast, and non-destructive.
- The original branch and commit are recorded and `git bisect reset` will run on every exit path.

If any condition fails, continue with local history and diff inspection.

**Gate:** The root cause and fix target are supported by concrete evidence, or a precise blocker is ready for the user.

## Phase 3 - FIX

State the diagnosis in one or two sentences, then apply the smallest coherent fix.

- Trace every changed line to the root cause.
- Preserve local conventions and unrelated user changes.
- Avoid opportunistic cleanup, broad refactors, dependency upgrades, suppressions, and speculative guards.
- Choose the least invasive correct strategy when several options are equivalent.
- Ask before changing a public API, architecture boundary, security policy, data model, or other hard-to-reverse contract.

Add or update one focused regression test when the bug is a non-trivial logic or runtime failure, an existing test harness is nearby, and the test directly reproduces the broken boundary. Do not add a redundant test for compiler-detected syntax or type errors.

Allow two fix attempts maximum:
1. Apply the best-supported strategy.
2. If focused verification fails and provides new evidence, use one fundamentally different strategy.

Never make a third attempt in the same trajectory. Stop with the evidence, attempted strategies, and remaining blocker.

**Gate:** The minimal fix is applied and ready for focused verification.

## Phase 4 - VERIFY/OUTPUT

Rerun the exact reproducer used in Phase 1.

Run at most one adjacent check only when it covers a distinct regression risk:
- A focused related test for changed shared behavior.
- A package-level type or build check when the reproducer did not compile the changed path.
- The newly added regression test when it differs from the original reproducer.

Do not automatically combine reproducer, full build, lint, type-check, and full test suite. Do not install packages, regenerate lockfiles, tidy manifests, start a development server, or modify environment state merely to verify a fix. Perform those actions only when they are the explicit reproducer or the user authorizes the required state change.

If verification fails, return to Phase 3 only when the two-attempt budget has room and the failure added discriminating evidence. Otherwise stop.

Return a compact receipt:
- Root cause and evidence.
- Files changed and why.
- Exact verification commands and results.
- Regression test added or intentionally omitted.
- Remaining blocker or uncertainty, if any.

Do not print the full hypothesis register when the fix succeeds. Do not add generic prevention advice.

**Gate:** The original failure is resolved by focused evidence, or the unresolved state is reported honestly after the capped attempts.

## Done When

- [ ] The failure was reproduced or a valid fresh baseline was accepted.
- [ ] The root cause is supported by local, documentation, or current external evidence.
- [ ] The fix is minimal and within scope.
- [ ] The original reproducer passes after the fix.
- [ ] At most one distinct adjacent check was added.
- [ ] Helper and fix-attempt caps were respected.

## Error Handling

| Scenario | Action |
|---|---|
| Reproducer unavailable | Ask for one missing command, input, environment fact, or state transition |
| Failure is intermittent | Preserve available evidence, investigate timing or state, and avoid claiming verification |
| Helper fails | Continue from sufficient local evidence or report the named gap |
| Root cause remains low-confidence | Stop before editing and ask one discriminating question |
| First fix fails | Use new evidence for one different strategy |
| Second fix fails | Stop and report both attempts with the remaining blocker |
| Verification requires state mutation | Request approval unless the action was already the explicit user-provided reproducer |

## Constraints

- **Always:** Reproduce or accept a fresh deterministic baseline, inspect locally first, keep hypotheses and helpers bounded, apply the smallest fix, and rerun the exact reproducer.
- **Never:** Launch every helper, run automatic `git bisect`, perform broad verification by default, install or regenerate dependencies for convenience, edit with low-confidence diagnosis, repeat the same failed strategy, exceed two fix attempts, or hide an unresolved failure.

## Examples

- `/meta-debug TS2322 in src/auth/session.ts after upgrading the auth package`
- `/meta-debug cargo test parser_handles_empty_input fails with an index panic`
- `/meta-debug The worker occasionally deadlocks after cancellation`

## Reference

- [Helper Protocols](references/helper-protocols.md): load only for delegated diagnosis.
