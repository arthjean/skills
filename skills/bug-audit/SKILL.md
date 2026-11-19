---
name: bug-audit
description: Proactive correctness audit that hunts unreported bugs (logic errors, edge cases, null or unexpected values, swallowed or misrouted errors, state inconsistencies, race conditions, resource leaks) and proves each one with a check that runs red. Use for bug hunts, correctness reviews of a module or flow, or a full codebase bug audit with report artifacts. Run the complete workflow only for explicit codebase audit requests, full or end-to-end reviews, or requested report artifacts.
---

# Bug Audit

Find defects that break a behavioral invariant on a reachable path, then give owners the source evidence, a failing check that proves the defect, its priority, and the smallest effective fix. This is a source-first, proof-by-execution workflow. A candidate without a named invariant, a concrete trigger, and an observed wrong result is not a confirmed finding.

Security exploitability is out of scope. A defect is in scope for its correctness consequence; when its main consequence is a crossed trust boundary, record a one-line hardening note for a security review and leave exploitability unassessed. A single failure someone already observed (an error, a stack trace, a failing test) is diagnosis, not audit: hand it to `meta-debug`.

## Operating modes

This skill is guidance by default. Loading it does not authorize the complete audit workflow or file creation.

- **Guidance mode**: For correctness questions, a focused review of one module, flow, or diff, triage of a suspected defect, or methodology, use only the relevant parts of this skill. Do not automatically run all six phases, create an output directory, or write audit artifacts. You may launch focused agents when useful; they return results to the current task.
- **Full audit mode**: Use the complete workflow when the user explicitly asks to bug-audit or bug-hunt a codebase, asks for a full, comprehensive, or end-to-end correctness review, or requests report artifacts. Run all six phases and write the files defined below.

If the request could mean either mode, ask one focused question before creating files or starting the complete workflow.

## Platform terminology

This skill is agent-neutral:

- **Parent** is the agent that coordinates the run and owns shared state.
- **Task tool** is the platform's delegation or sub-agent mechanism.
- **`research` agent** is a delegated read-only agent for focused source exploration and factual verification.
- **`general` agent** is a delegated agent for broad investigation and bounded local execution.
- **`subagent_type:`** in a heading names which of these two delegated agent roles runs that work.

In Claude Code, `research` maps to a read-only agent type such as `Explore`, and `general` to `general-purpose`. Use equivalent platform capabilities elsewhere while preserving role, write-isolation, prompt, and independence boundaries.

## Execution policy

Proof by execution is the evidence bar, so this skill runs target code far more than a security audit does. Before reconnaissance, pick one policy and record it in `run-metadata.json` as `execution_policy`:

- **`sandboxed`**: an OS-enforced sandbox provides no external network (an isolated loopback namespace is allowed), writes confined to the agent's `scratch/`, an allowlisted environment, and explicit CPU, memory, process, disk, and wall-clock limits. Any check may run inside it.
- **`trusted-local`**: no OS sandbox is available, and the user has confirmed that the target is their own or otherwise trusted code. Run only checks whose reach you have read: the agent's own tests and harnesses, and existing test files or suites whose setup, fixtures, and environment configuration you have read and that reach nothing beyond loopback.
- **`source-only`**: neither applies. Nothing executes; every candidate that needs runtime proof ends as `needs_validation` with the blocker `execution_policy_source_only`.

Ask the user to confirm trust before choosing `trusted-local`; never infer it from the repository owner or location. These rules bind every executing policy:

1. **Disposable copy.** Target source, its `.git`, and its working tree stay untouched. Each executing agent exports the audited tree into its own `scratch/` (`git archive` of the audited ref, or a copy of tracked plus unignored untracked files for a dirty tree) and points build outputs and caches there (`CARGO_TARGET_DIR`, compiler `outDir`, test caches). Reuse already-installed dependencies read-only by linking the existing `node_modules`, virtual environment, or vendor directory. Never install or fetch dependencies; a check that needs a fetch is blocked.
2. **Clean environment.** Pass only toolchain variables (`PATH`, toolchain homes, locale) plus values the check sets on purpose. Never pass credentials, tokens, connection strings, or cloud profiles, so a test that needs a real service fails closed instead of reaching it.
3. **Local reach only.** Loopback services the check starts itself, in-memory or file-backed fakes, and local containers already present and started for this check only. Never deployed endpoints, shared databases, queues, paid APIs, email or notification providers, or other people's data.
4. **Bounded runs.** Every command has a wall-clock timeout. Stress, fuzz, and property loops have explicit iteration and time bounds. Stop at the minimum observation that settles the claim.
5. **Pinned determinism.** Pin time zone, locale, random seed, and clock when the check depends on them, and record each pin. When the defect lives in one of those values, set the triggering value explicitly instead.
6. **No outward effects.** Never commit, push, tag, publish, deploy, run migrations outside scratch, or modify anything outside scratch.

## Full audit setup

In full audit mode, resolve these values before reconnaissance:

- **Skill directory**: the absolute directory containing this `SKILL.md`.
- **Target**: the absolute repository root under review.
- **Repo name**: a stable repository identifier from the directory or local Git remote.
- **Output directory**: a new writable directory outside the target, defaulting to `~/bug-audit/<repo-name>/run-<N>`, where `<N>` is the next unused integer. Use a directory inside the target only when the user explicitly selects it and the parent verifies that version control ignores the whole directory. Otherwise stop and request an external path.
- **Source ref**: the reviewed commit and whether the worktree is dirty. Do not treat unreviewed generated or modified files as another revision.
- **Execution policy**: one of the three policies above.

### Write isolation

The parent creates and is the only writer of shared run files:

- `run-metadata.json`
- `architecture.md`
- `coverage-ledger.json`
- `findings.json`
- `REPORT.md`
- `FINDINGS-DETAIL.md`
- `NEEDS-VALIDATION.md`

Each hunter or verifier receives a unique root under `<output-dir>/agents/<agent-id>/`, with separate `scratch/` and `artifacts/` directories. Canonical agent IDs match `^[a-z0-9][a-z0-9_-]{0,63}$` and must not equal a Windows device name such as `con`, `prn`, `aux`, `nul`, `com1` through `com9`, or `lpt1` through `lpt9`. The agent and every process it starts write only to `scratch/`, which also holds its disposable copy of the target. Retained `artifacts/` is parent-owned. Agents never change shared files, target source, retained artifacts, or another agent's directory, and never use `/tmp` or the host home directory as a writable fallback.

Before an agent executes anything, the parent records an allowlist of expected scratch-relative artifact files (red checks, harnesses, captured output) with per-file and cumulative byte limits. After the agent and all its processes end, the parent promotes each allowlisted file separately:

1. Reject an absolute or empty path, a `.` or `..` component, and any component that is a symlink, junction, or reparse point.
2. Require a regular file with a link count of exactly one, within its recorded byte limits.
3. Copy exactly that file, then re-check its size and identity; reject a file that changed during the copy.
4. Create the destination exclusively under `agents/<agent-id>/artifacts/`; never overwrite, glob, recurse, or extract an archive into artifacts.
5. On any failure, discard the entry; if it was decisive evidence, the candidate keeps a `needs_validation` blocker naming the promotion failure.

[HUNTING.md](HUNTING.md) and [VALIDATION-AND-REPORTING.md](VALIDATION-AND-REPORTING.md) carry this procedure as one identical fenced block for hunter and verifier prompts.

For a reproduced check, record the command, exact input or seed, timeout and limits, run count, and only the allowlisted environment variable names plus the safe values needed to reproduce it. Never capture or copy the ambient environment, inherited variables, credential values, or unrelated host paths.

Before delegation, the parent writes `run-metadata.json` with at least `run_id`, `repo`, `target`, `source_ref`, `profile`, `scope_paths`, `budget` (null if unset), `execution_policy`, the user's trust confirmation when the policy is `trusted-local`, selected companion files, prior-run paths, shared-file owners, and `run_status: "in_progress"`. Update metadata only when those facts change; candidate state belongs in the coverage ledger and `findings.json`.

## Full audit planning

The coverage, prior-run, profile, and budget requirements in this section apply only in full audit mode.

### Coverage and prior runs

No one pass is complete. Build a deterministic coverage plan before hunting and update it after every agent result. [RECONNAISSANCE.md](RECONNAISSANCE.md) defines the stable coverage units and [HUNTING.md](HUNTING.md) defines coverage-critic waves. The parent alone updates the ledger.

If prior runs exist, read every compatible `coverage-ledger.json` and `findings.json` before planning the current run:

1. Compare the relevant current source with each prior record and unit. A prior source ref alone is not evidence that a path is unchanged.
2. Carry a prior `confirmed` record into the current candidate set only when its relevant source and conditions are unchanged and its evidence still meets the current contract. Link it to a current ledger unit seeded `planned`, preserve its fingerprint, exclude only that carried root cause from hunters, and send the carried record through the current final verification path, which re-runs its regression test; the Phase 3 verifier that re-checks it becomes that unit's assignment owner and moves it to `candidate`.
3. When relevant source for a prior `confirmed` record changed, create a current planned revalidation unit. Do not put that record on the hunter exclusion list. It remains confirmed only if its regression test still runs red on current source under independent validation; a green run is evidence of a fix, recorded as `rejected` with reason `fixed_in_current_source`.
4. Make prior `needs_validation`, `deferred`, `blocked`, `out_of_scope`, and any changed-source unit current work. A still-blocked `needs_validation` record may be carried only after its current source trace is checked and linked by fingerprint to a current `planned` unit whose verifier re-check supplies its owner and evidence; the record keeps the unresolved blocker. These prior states never suppress a current unit.
5. A prior same-source covered unit may inform priority, but it remains visible in the current ledger. A prior `rejected` record suppresses only the unchanged failed claim, not coverage of its unit; changed evidence creates current work.
6. Read the prior profile and scope. A prior `quick` or scoped ledger contributes only its recorded evidence and gaps, never an implied "rest is fine."

If no prior ledger exists, say so in the final coverage statement. Never imply that one run exhausts the target.

### Run profiles and scope

During full audit setup, pick a profile from the user's request or propose one from the target's size and stakes. Record it in `run-metadata.json` (`profile`, `scope_paths`) and state it in the report. The default is `standard`.

- **`quick`**: a bounded pass for small targets, re-runs, or a fast first look. Coarsen ledger units to entry × invariant × bug class (subsystem uses the fixed canonical `profile/quick/all-in-scope-subsystems` identifier), run exactly one hunter wave followed by exactly one final coverage-critic pass, and use one fresh verifier per candidate for both candidate validation and final record verification. Do not launch a follow-up hunter wave: record the critic's accepted discoveries and reassignments as `deferred`.
- **`standard`**: the workflow as written.
- **`deep`**: for high-stakes or large targets. Split ledger units per subsystem and lifecycle mode, run critic waves to a clean pass, keep candidate validation and final record verification as separate fresh agents, and give `prior_covered_same_source` units an independent second pass.

A **scoped run** audits a subset: named paths, one subsystem, one companion domain, or the diff between two source refs. Seed ledger units only for in-scope entries and record everything else as `out_of_scope`, never as `covered`. A scoped or `quick` run must present itself as partial coverage.

Profiles change breadth and redundancy, never the evidence bar. Do not scale away the candidate gate, the execution policy, `needs_validation` discipline, schema validation, or independent verification of `confirmed` records.

#### Cost budget

The ledger makes spend countable: one unit is roughly one hunter assignment, and one surviving candidate is one or two verifier assignments depending on profile. When the user sets a budget, or the parent proposes one for a large target, record `budget` in `run-metadata.json` as a maximum number of agent invocations across all phases.

Apply the strict budget gate before launching any reconnaissance agent. Reserve the four baseline reconnaissance calls, one final post-wave critic for `quick` or one post-wave plus one distinct final-clean critic for `standard`/`deep`, and at least one verifier call. Add focused reconnaissance only after repeating this gate for each extra call. If the requested budget cannot fund that minimum, launch no agent: ask for a larger budget, narrower scope, or different profile. If the request remains unchanged, set `run_status: "incomplete"` with `incomplete_reason: "budget_cannot_fund_reconnaissance_and_reserves"` and report that no audit pass ran.

Spend it in this order:

1. Count reconnaissance, every post-wave critic, and the separate final-clean critic as agent invocations.
2. **Reserve critics and validation before hunting.** For `quick`, reserve its one post-wave final critic. Before every `standard` or `deep` hunter wave, reserve one immediate post-wave critic plus one distinct final-clean critic. Also reserve verifier cost from the profile (about 1 or 2 agents per expected candidate; when in doubt reserve 30% of the balance after critic reservation). Never assign hunters into either reserve.
3. Assign hunters to units in priority order until the hunting allowance is spent. Spend the reserved post-wave critic immediately after that wave; keep the final-clean and validation reserves intact.
4. Before a later wave, reserve its new post-wave critic again. If the remaining budget cannot cover the required critic calls and validation reserve, launch no hunters from that wave, mark its planned units `deferred` with reason `budget_cannot_reserve_critics_and_validation`, and use the retained final-clean critic to record the resulting gap.

Before wave 1, update the pre-reconnaissance estimate with seeded units, implied hunter count, mandatory critic calls, validation reserve, and whether the remaining budget covers the plan. If it clearly cannot, say so and propose either a tighter scope or a coarser profile instead of silently thinning evidence. If later facts consume the required final-critic reserve, launch no hunters, mark all planned work deferred, set the run incomplete with reason `critic_budget_exhausted`, and make no complete-coverage claim.

A strict total-agent budget can still be exceeded by an unexpectedly large candidate set or by a material Phase 5 replacement that needs another independent verifier. If the remaining budget cannot validate every candidate, stop hunting, validate candidates in fingerprint order while the budget permits, and set `run_status: "incomplete"` plus `incomplete_reason: "validation_budget_exhausted"`. Keep each unvalidated fingerprint linked to a `candidate` ledger unit with that unresolved reason. Do not put an unvalidated candidate in `findings.json`, relabel it `needs_validation`, or report the run as complete. Phase 6 may produce a partial report only if its first section states that candidate validation is incomplete and lists the affected fingerprints and units. Never exceed a user-set strict budget silently.

## Core principles

### Require an invariant and a wrong result

For every candidate, name the **invariant** (the contract the code must keep, and its source: a type, a schema constraint, a documented rule, a test, a caller's assumption, or evident intent), the **trigger** (input value, prior state, interleaving, dependency failure, configuration, or environment), the reachable path from a real entry, and the **observed wrong result** (wrong value, crash, hang, leak growth, lost or misattributed error, or inconsistent persisted state). A style choice, a defensive check no reachable trigger needs, a value the types or every caller already exclude, and documented or tested intended behavior are not findings.

### Prove by execution

A `confirmed` record carries a check that ran **red**: it fails on the current source for the stated reason and would pass once the invariant holds. Where practical, pair it with a **control**, the same check passing with the trigger removed (valid input, serialized schedule, healthy dependency), so the pair shows that the trigger causes the failure. Pick the narrowest proof that reaches the defect:

| Defect signal | Proof kind |
|---|---|
| Wrong result for specific inputs | `unit_test` or `integration_test` through the real seam |
| Wrong result across an input domain | `property_test` with a shrunk counterexample, using the project's property library or one already installed |
| Crash or hang on malformed input | bounded `fuzz` run or a minimal fixture in a `unit_test` |
| Race, ordering, or lost update | `deterministic_schedule` (barriers, injected yields, a controlled executor, `loom`), else a bounded `stress_loop` reporting its reproduction rate |
| Swallowed or misrouted error | `fault_injection` at the dependency seam, asserting what the caller observes |
| Resource leak | `leak_measurement`: repeated iterations showing monotonic growth of handles, memory, listeners, tasks, or pool checkouts after cleanup should have run |
| Undefined behavior or memory error in native code | `sanitizer` run (ASan, UBSan, TSan, LSan) or Miri |
| Behavior that diverges between versions or configurations | `differential` run on identical input |
| No seam reaches the path | minimal `harness` that calls the real code with the trigger |

A check that passes, fails for a different reason, or fails only because the harness itself is wrong does not prove the candidate. For a non-deterministic defect, record runs executed and failed; the rate must be high enough to separate the defect from noise, and the control must stay green across the same number of runs. The red check becomes the record's `regression_test`.

### Respect what source cannot show

Production data shape, real traffic concurrency, hardware and platform behavior, third-party service behavior, and deployed configuration are real conditions. If one is decisive and cannot be reproduced locally, do not assume it either way. Use `needs_validation` with the exact missing fact and a safe owner-observed check (a log or metric query, a read-only data query, a platform test the owner runs).

### Separate priority from certainty

Only `confirmed` records receive severity. Likelihood is how often realistic use meets the trigger; impact is the consequence the proof demonstrates. Overall severity cannot exceed demonstrated impact. `needs_validation` means a specific source-grounded defect hypothesis whose proof is blocked; it has no severity.

Calibrate overall severity with these anchors:

- **critical**: silent loss or corruption of persisted or shared data, or a failure that takes the whole system down or blocks its recovery, on a realistic path.
- **high**: wrong results or a crash on a common path; persisted inconsistent state that needs manual repair; a failure reported as success where the caller acts on it.
- **medium**: wrong behavior under uncommon but realistic conditions (specific input, timing, dependency failure), recoverable by retry, restart, or a user action; a leak that degrades a long-running process over hours or days.
- **low**: wrong behavior with a cheap workaround, wrong cosmetic output, or a leak bounded by a short process lifetime.
- **informational**: a confirmed but negligible defect, useful mainly as a prerequisite inside a larger finding.

The high/medium discriminator: does the wrong result persist or propagate beyond the operation that caused it (stored, sent, acted on, or masking a failure), or does it stay contained and recoverable? If you cannot state who or what is harmed by the wrong result, the severity is lower than it feels.

### Recommend the smallest effective fix

For each confirmed finding, identify the invariant and the narrowest source change that enforces it where it is first violated, with the regression test that proves it. Prefer a fix at the root over a guard at the symptom: a null check, default value, or catch that hides an upstream defect moves the bug instead of fixing it. The audit describes fixes; it does not modify target source. Fixing is a separate step, and each confirmed record's regression test is the reproducer `meta-debug` needs.

## Full audit workflow

In full audit mode, follow all six phases in order:

1. **Reconnaissance**: map the stack, invariants and their enforcement, entries and flows, tooling signals and hotspots, prior evidence, companion selections, and the initial deterministic coverage ledger with [RECONNAISSANCE.md](RECONNAISSANCE.md).
2. **Coverage-led hunting waves**: assign isolated hunters from the ledger and collect structured candidate results with [HUNTING.md](HUNTING.md), [BUG-CLASSES.md](BUG-CLASSES.md), and the selected domain companions.
3. **Candidate validation**: consolidate fingerprints and give every candidate to a fresh verifier that re-runs the red check, as defined in [VALIDATION-AND-REPORTING.md](VALIDATION-AND-REPORTING.md).
4. **Structured output**: write all final `confirmed`, `needs_validation`, and `rejected` records to `findings.json`; validate it with `report-schema.json` and `validate-findings.cjs`, and validate the coverage claim with `validate-coverage-ledger.cjs`.
5. **Independent record verification**: use fresh agents to verify final source claims and re-run each regression test from the record alone.
6. **Target-neutral report**: derive `REPORT.md`, `FINDINGS-DETAIL.md`, and `NEEDS-VALIDATION.md` from the final records.

Do not end the run before one of exactly two terminal states: (a) all Phase 6 artifacts are written and both validators pass, or (b) `run_status: "incomplete"` is recorded with its exact reason and the gap is disclosed in the report. Never stop mid-phase.

## Anti-patterns

1. Style, naming, or maintainability notes presented as bugs.
2. "Could be null" or "might race" without a path that delivers the value or a schedule that shows the interleaving.
3. Defensive-programming advice with no reachable failure.
4. Documented or tested intended behavior reported as a defect.
5. A leak claimed without growth across repeated iterations, or a hang claimed from a slow path.
6. An observed effect reported stronger than observed: a logged warning as data loss, one wrong field as corruption.
7. A red check that fails for a reason other than the claimed defect.
8. Prose-only hunter results that cannot be deduplicated or verified.
9. Re-reporting carried same-source prior confirmed records or using them as exemplars that anchor the hunt.
10. Assigning severity to `needs_validation` records.
11. Editing target source during the audit, or writing the report before independent verification.
