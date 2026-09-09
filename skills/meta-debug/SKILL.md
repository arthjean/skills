---
name: meta-debug
description: Reproduce-first diagnosis and minimal fix of an observed failure, two fix attempts maximum. Use when the user gives an error, stack trace, failing test or command, or asks to debug a concrete failure.
---

# meta-debug

Find the root cause of the reported failure, apply the smallest safe fix, and prove it with the original reproducer. Explicit user instructions override anything below.

## Reproduce

Pick the smallest deterministic reproducer: the named failing test, compiler target, command, request, or script. A complete fresh compiler, type-checker, or test output already in hand is a valid baseline; otherwise run the reproducer once. Never a full suite, broad build, lint, or type-check when a smaller command reproduces the failure. Record the core error and the command, not the transcript.

If it does not reproduce, do not guess: check for intermittence or environment dependence, then ask one targeted question for the missing state, input, timing, or command.

## Diagnose

Local evidence first: the failing file and the smallest surrounding function, type, fixture, or config; user-code stack frames before framework or generated ones; targeted `rg` on the failing symbol, error code, or key; the manifest or lockfile only when a version or setup is a plausible cause. Prefer observations that eliminate several explanations at once. Keep at most three or four live hypotheses, internally, and surface them only if diagnosis stalls.

Delegate only a named gap local inspection cannot settle, at most two helpers per run, read-only: `agent-explorer` after focused reads leave a cross-module, concurrency, generated-boundary, or state interaction unresolved; `docs-researcher` for one exact version-sensitive API or configuration question; `web-researcher` for a plausible known external bug or undocumented behavior once local and official evidence are exhausted. See [Helper Protocols](references/helper-protocols.md) when delegating. A failed helper is not retried through another role.

Regression history: read-only `git log`, `git show`, `git diff`, `git blame`. `git bisect` only with explicit user approval, a clean or isolated worktree, known good and bad bounds, a fast deterministic non-destructive reproducer, and `git bisect reset` on every exit path.

The root cause is clear when direct evidence supports one causal explanation and one fix target. Eliminating every theoretical alternative is not required. Ask one discriminating question rather than edit on a low-confidence diagnosis.

## Fix

State the diagnosis in one or two sentences, then apply the smallest coherent fix: every changed line traces to the root cause, local conventions and unrelated changes preserved, no opportunistic cleanup, suppressions, dependency upgrades, or speculative guards. Ask before changing a public API, architecture boundary, security policy, data model, or other hard-to-reverse contract.

Add one focused regression test when the bug is a non-trivial logic or runtime failure, a harness is nearby, and the test reproduces the broken boundary. Compiler-detected syntax or type errors get none.

Two attempts maximum: the best-supported strategy, then one fundamentally different strategy if verification fails with new evidence. Never a third. Stop with the evidence, both strategies, and the remaining blocker.

## Verify

Rerun the exact reproducer. Add at most one adjacent check when it covers a distinct regression risk: a related focused test, a package-level type or build check when the reproducer did not compile the changed path, or the new regression test. Rerunning the reproducer and these focused checks is authorized without asking. Installing packages, regenerating lockfiles, starting servers, or mutating environment state needs authorization unless it was the user-provided reproducer itself.

Receipt: root cause and evidence, files changed and why, exact verification commands and results, regression test added or intentionally omitted, remaining blocker. No hypothesis register on success, no generic prevention advice.

**Complete when:** the original reproducer passes after a fix whose every line traces to the root cause, or the unresolved state is reported after the capped attempts.

## Examples

- `/meta-debug TS2322 in src/auth/session.ts after upgrading the auth package`
- `/meta-debug The worker occasionally deadlocks after cancellation`
