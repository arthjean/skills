# Bug Hunting

### Phase 2: Run coverage-led hunting waves

The parent assigns `planned` ledger units to `general` agents. Use enough focused hunters to cover the units without combining unrelated invariants. One hunter may own closely related units in one subsystem; no unit may be silently unassigned because of an agent-count limit: a unit the budget cannot reach is explicitly `deferred` with reason `budget_cannot_reserve_critics_and_validation`.

When a budget or profile caps hunter count, assign units in priority order and record the ordering rationale in the ledger. Rank by: (1) paths that write persistent or shared state, or act on a reported success, before read-only paths; (2) invariants that are implied only or documented but neither enforced nor tested; (3) Agent 1d hotspots (change frequency, bug-fix history, failing or flaky tests, risk-signal density, low coverage); (4) prior-run gaps, revalidation targets, and changed source before same-source re-passes; (5) classes that historically yield confirmed findings for this stack. Ties break lexicographically by `coverage_id` so runs stay deterministic.

Before launch, the parent changes assigned units to `in_progress`, sets a canonical lowercase `agent_id`, and creates that agent's `scratch/` and parent-owned `artifacts/`. Hunters read source and parent-provided context, build their disposable copy and checks only in their unique `scratch/`, and return one structured result through the Task tool. They never write retained artifacts or edit target source, `architecture.md`, `coverage-ledger.json`, `findings.json`, or another agent's files.

## Required hunter prompt

Every hunter prompt contains these parts in this order:

1. A two-sentence role preamble: the hunter's goal is to find reachable violations of behavioral invariants in its assigned units and prove each with a check that runs red, and it must return exactly one JSON object matching the structured-result contract at the end of this prompt.
2. `architecture.md` verbatim.
3. Assigned coverage IDs, subsystem, invariant, repository-relative starting paths, and each unit's assignment block map from `coverage-ledger.json`.
4. The exact selected blocks, copied verbatim: each selected core bug-class block from `BUG-CLASSES.md`, and from each selected companion its `Core discipline`, each chosen bug-class subsection, `Universal moves`, and `Validation rules`. Core blocks are self-contained and carry no companion-style `Core discipline`, `Universal moves`, or `Validation rules` sections. Do not send block or companion names alone.
5. Explicit excluded core and companion blocks with a reason for each exclusion.
6. The core hunting method below, followed by the execution policy block with the run's policy filled in, then the promotion procedure block.
7. The core validation rules below.
8. Carried same-source prior confirmed exclusions, each limited to fingerprint, title, and root cause, plus peer-owned current coverage IDs that this hunter must not duplicate.
9. The unique scratch and artifact paths, safe agent ID, predeclared promotion allowlist and byte limits, and the structured-result contract, including the Structured hunter result block below and the `confirmed` and `needs_validation` branches of `report-schema.json` copied verbatim.

A prompt may select several companion blocks when the same path crosses several domains. Keep their constraints together. Scope is the hunter's coverage obligation, not permission to duplicate excluded work. If an unexpected different invariant or entry appears, return it under `uncovered` so the parent creates a stable ledger unit and assigns it in the next wave.

#### Core hunting method: include in every hunter prompt

```text
## Bug-finding method

Your goal is to find reachable violations of behavioral invariants, prove each with a
check that runs red, and state the smallest fix. Stay within source review and the
run's execution policy. Never edit target source. Do not contact deployed endpoints,
shared databases, queues, paid or notification providers, or anyone's real data.

READ THE CODE AT DEPTH. Follow each assigned entry through parsing, validation,
transformation, state reads and writes, external calls, and the final effect. Read
sibling, legacy, batch, retry, cancellation, migration, startup, shutdown, and error
paths that produce the same effect. Compare what each function guarantees with what
its callers assume: nullability, units, encodings, ordering, time zone, error mode,
and ownership of returned resources.

WORK FROM A CONCRETE INVARIANT:
1. State the invariant and its source: a type, schema constraint, documented rule,
   test, caller assumption, or evident intent.
2. Name the trigger: input value, prior state, interleaving, dependency failure,
   configuration, or environment.
3. Trace the exact path from a real entry to the point where the invariant breaks.
4. State the observable wrong result: wrong value, crash, hang, leak growth, lost or
   misattributed error, or inconsistent persisted state.
5. Build the narrowest check that runs red for that reason and, where practical, its
   control: the same check green with the trigger removed.
6. State the source-level fix and the regression test that enforces the invariant.

DEPTH BOUND: trace only paths that can reach your assigned invariant or whose
guarantees that invariant relies on. Stop a line of investigation as soon as the
invariant is settled either way, and record the result in your structured output (a
covered, candidate, or blocked disposition, or an `uncovered` entry) instead of
continuing to search.

TEST THE SAD PATHS. For every accepted input or state, try absent, null, empty, zero,
negative, one, maximum, maximum plus one, NaN and infinities, duplicate, unsorted,
non-ASCII text (combining marks, emoji, right-to-left), very long, whitespace-only,
stale, already deleted, concurrently modified, partially migrated, timed-out
dependency, dependency failing after partial work, retried, and canceled, wherever
the interface accepts them. At every handoff, compare units, encodings, rounding, and
time zones.

FOLLOW EVERY ERROR. For each failure a callee can produce, find where it is caught,
transformed, logged, retried, or dropped, and what the caller then believes. Success
reported after a failure, an error mapped to the wrong type or status, a retry that
repeats a non-idempotent effect, and cleanup skipped on the error path are candidates.

FOLLOW EVERY RESOURCE. For each acquire (file, socket, connection, cursor, lock,
transaction, timer, listener, subscription, task, thread, child process, temporary
file, cache entry), find the release on the success path, every error path, every
early return, and cancellation.

FOLLOW EVERY SHARED VALUE. For each value two executions can touch (memory, row, file,
cache, global, module state, UI store), find its guard, the transaction or lock scope,
and what happens when two executions interleave at each await, yield, callback, or
lock release.

When a candidate reveals a reusable root cause, search paths owned by the assigned
coverage IDs for lexical, structural, and logical variants: the same pattern at other
call sites, the copy that was not fixed, the sibling path with the same assumption.
Consolidate the same root cause, but establish each variant's trigger and wrong result
independently. Do not investigate peer-owned units. Return a variant with no current
coverage unit as `uncovered`.

USE THE NARROWEST CHECK THAT SETTLES THE CLAIM. Execute only as the execution policy
block in this prompt allows, inside your disposable copy. Prefer, in order: one new
case in an existing test file, a minimal harness calling the real function, a property
test with a fixed seed, a deterministic schedule, a fault-injecting fake at the
dependency seam, a bounded repetition loop measuring growth or reproduction rate. Do
not install or fetch tools. Read the failure output: a red run proves the candidate
only when it fails for the stated reason.

Record the exact command, input or seed, timeout, runs executed and failed, red
output, and control output. For the environment, record only allowlisted variable
names and safe values needed to reproduce the check. Write checks and captured output
only in scratch and list the files to promote; the parent promotes them by the
promotion procedure block included verbatim in this prompt. If promotion is
unavailable or fails for decisive evidence, return needs_validation with the exact
promotion blocker.

A production data, real traffic, hardware, platform, third-party, or deployed
configuration fact outside source is not proof either way. If one such fact is
decisive, return a needs_validation record with the exact missing observation and a
safe owner-observed check.
```

#### Execution policy block: copy verbatim, with the run's policy filled in, into every hunter and verifier prompt

```text
Execution policy for this run: <sandboxed | trusted-local | source-only>

sandboxed: an OS-enforced sandbox blocks external network (isolated loopback allowed),
confines writes to your scratch, and enforces CPU, memory, process, disk, and
wall-clock limits. Any check may run inside it.
trusted-local: no OS sandbox; the user confirmed the target is trusted. Run only checks
whose reach you have read: your own tests and harnesses, and existing test files or
suites whose setup, fixtures, and environment configuration you have read and that
reach nothing beyond loopback.
source-only: execute nothing. Every candidate that needs runtime proof ends as
needs_validation with the blocker execution_policy_source_only.

Under sandboxed and trusted-local:
1. Work in a disposable copy of the audited tree in your scratch. Never touch target
   source, its .git, or its working tree. Point build outputs and caches into scratch.
   Reuse installed dependencies read-only by linking them. Never install or fetch; a
   check that needs a fetch is blocked.
2. Pass only toolchain variables plus values the check sets on purpose. Never pass
   credentials, tokens, connection strings, or cloud profiles.
3. Reach only loopback services the check starts itself, in-memory or file-backed
   fakes, and local containers already present and started for this check.
4. Give every command a wall-clock timeout and every loop explicit iteration and time
   bounds. Stop at the minimum observation that settles the claim.
5. Pin time zone, locale, random seed, and clock when the check depends on them, and
   record each pin; set the triggering value explicitly when the defect lives in it.
6. Never commit, push, tag, publish, deploy, run migrations outside scratch, or modify
   anything outside scratch.
```

#### Promotion procedure: copy this promotion procedure verbatim into every hunter prompt

```text
Artifact promotion procedure (parent only):
Reference only for you: the parent performs these steps; you never perform them.

Before you execute anything, the parent records an allowlist of expected
scratch-relative artifact files with per-file and cumulative byte limits. After you and
all your processes end, the parent promotes each allowlisted file separately:

1. Reject an absolute or empty path, a `.` or `..` component, and any component that
   is a symlink, junction, or reparse point.
2. Require a regular file with a link count of exactly one, within its recorded byte
   limits.
3. Copy exactly that file, then re-check its size and identity; reject a file that
   changed during the copy.
4. Create the destination exclusively under agents/<agent-id>/artifacts/; never
   overwrite, glob, recurse, or extract an archive into artifacts.
5. On any failure, discard the entry; if it was decisive evidence, the candidate keeps
   a `needs_validation` blocker naming the promotion failure.
```

#### Core validation rules: include in every hunter prompt

```text
## Candidate gate

1. A candidate needs a complete repository-relative trace from a real entry to the
   failure point, the invariant it breaks, and evidence for the root cause, including
   the strongest source-visible guard (type, validation, constraint, lock, catch) that
   fails to prevent it.
2. A proposed confirmed record needs a check that ran red for the stated reason,
   complete conditions, and no visible layer (callers, types, schema, framework) that
   keeps the trigger from occurring in real use.
3. Report the observation at its true strength: a logged warning is not data loss, a
   slow path is not a hang, one extra allocation is not a leak, and an interleaving is
   not a race until a schedule or measured rate shows it.
4. If a decisive fact is not source-visible or locally reproducible, use
   needs_validation. Name exact blockers; give it no severity and no speculative
   completion.
5. A style issue, a missing best practice, or a defensive check with no reachable
   trigger is excluded or a hardening note, not a finding. Documented or tested
   intended behavior is not a defect; a contradiction between documentation and code
   is a candidate only when a caller relies on the documented behavior. A candidate
   disproved by source is not needs_validation.
6. Use the same source-derived fingerprint for the same root cause in every state.
   It must match `^[A-Za-z0-9][A-Za-z0-9._:/@+-]*$` and must not include a line,
   wave, agent, severity, or verdict.
7. Return an empty candidate array when nothing survives these gates.
```

## Proof boundaries

Local execution is for proof, not for exploration at scale:

- **Allowed under an executing policy:** offline builds with present dependencies; existing and new unit, integration, and property tests; minimal harnesses; bounded fuzz runs; deterministic schedules and bounded stress loops; fault-injecting fakes; leak measurement over bounded iterations; sanitizers and Miri; loopback processes the check starts itself with dummy data; differential runs between refs or configurations inside scratch.
- **Disallowed:** traffic to deployed or shared services; dependency installation or network fetches; real accounts, credentials, or production data; shared databases, queues, caches, runners, or registries; publishing, deploying, or committing; load generation beyond the bounded loop the claim needs; any work after the observation that settles the claim.

Missing an executing policy, a toolchain, or a promotion capability does not erase a source-grounded candidate; represent the exact blocker in `needs_validation`.

## Structured hunter result

Return exactly one JSON object, with no surrounding prose:

```json
{
  "units": [
    {
      "coverage_id": "one assigned ID",
      "disposition": "covered|candidate|blocked",
      "reviewed_paths": ["repo/relative/path"],
      "checks": [
        {
          "agent_id": "canonical owner of this check",
          "reviewed_paths": ["repo/relative/path owned by this check"],
          "invariant": "specific invariant checked for this unit",
          "method": "source|local",
          "result": "what source or the bounded check established",
          "artifact": "agents/<agent-id>/artifacts/file for local, null for source"
        }
      ],
      "candidate_fingerprints": [],
      "unresolved": []
    }
  ],
  "candidates": [],
  "promote": ["scratch-relative file to promote"],
  "hardening": ["concrete robustness note that is not a finding"],
  "uncovered": [
    {
      "entry": "...",
      "invariant": "...",
      "subsystem": "...",
      "bug_class": "...",
      "starting_paths": ["repo/relative/path"],
      "reason": "why this needs its own deterministic coverage unit"
    }
  ]
}
```

Each `candidates` entry is schema-shaped except that it uses `proposed_verdict` in place of `verdict`:

- `proposed_verdict: "confirmed"`: include every field required by the `confirmed` branch of `report-schema.json` other than `verdict`: `fingerprint`, `title`, `description`, `bug_class`, `invariant`, `root_cause`, ordered `trace`, `evidence`, `conditions`, `reproduction`, `regression_test`, `remediation`, `severity`, and `confidence`. `reproduction` describes only the check already run: `inputs` holds the minimum input, fixture, seed, or schedule; `observed_result` and `runs` record actual output and counts; `control_result` records the control when one ran. `regression_test.code` is the red check itself, written to live in the target's test layout at `regression_test.file_name`. Overall severity must not exceed observed impact.
- `proposed_verdict: "needs_validation"`: include every field required by that schema branch other than `verdict`: `fingerprint`, `title`, `description`, `bug_class`, `invariant`, `claimed_root_cause`, ordered `trace`, `evidence`, nonempty `blockers`, and `validation_plan` with at least one applicable `local` or `owner` step. Do not invent an inapplicable context. Do not include severity, reproduction, regression test, remediation, confidence, reason, or a confirmed `root_cause`. `owner` is a check the owner performs (a log or metric query, a read-only data query, a platform test), not a request to probe a live system.

Every assigned coverage ID appears exactly once in `units`. A `covered` unit needs an owner, nonempty `reviewed_paths` and `checks`, no unresolved fact, and no candidate. A `candidate` unit has the same owned evidence and is the only state that carries linked fingerprints. A `blocked` unit is an owned partial review with nonempty paths, checks, and unresolved facts but no fingerprint. All source paths are repository-relative, never absolute or traversal paths. A trace with several entries begins at `entrypoint`, ends at `failure`, and labels intermediate steps `propagation`. Every check has its own canonical lowercase `agent_id` and nonempty `reviewed_paths`; the unit-level list is exactly the union of those owned paths. A `source` check uses `artifact: null`. A `local` check uses one file the parent successfully promoted beneath `agents/<check.agent_id>/artifacts/`; this permits a verifier to add independently owned evidence without taking ownership from the hunter. Never link scratch, an output-root file, or another check owner's artifact.

## Parent consolidation and ledger update

The parent promotes each file listed in `promote` that the allowlist covers, then validates each unit result, maps it to exactly one assigned `coverage_id`, and updates only that ledger unit. Reject duplicate or absent IDs, unsafe unit or check agent IDs, source checks with artifacts, and local artifacts the parent did not promote into the check owner's artifacts subtree. Copy the unit's `reviewed_paths`, its `checks` into the unit's `local_checks`, linked artifact paths, candidate fingerprints, and unresolved facts into the ledger. Retain each hunter's `hardening` list in a parent bookkeeping field on the relevant units (outside the semantic fields) so Phase 6 can report it. A failed, malformed, or unsupported conclusion leaves that unit `planned` for reassignment. Untouched budget or profile units become unassigned `deferred` units with empty evidence and a reason; do not hide partial evidence in `deferred`. Run `validate-coverage-ledger.cjs` after the update; an invalid ledger cannot drive another assignment. This per-unit contract allows one hunter to close one unit while returning a candidate or blocker for another.

Consolidate candidate entries by fingerprint and then by root cause. One root cause that surfaces through several entries or effects is one candidate with the strongest complete trace and proof. Related but independent defects use separate fingerprints. Record duplicate fingerprints in the relevant ledger unit and do not send duplicate candidates to validation.

## Coverage-critic waves

Immediately after each hunter wave, spend the reserved invocation on one fresh `research` post-wave coverage critic. It receives `architecture.md`, the full coverage ledger including each assignment block map, current candidate fingerprints and states, the Agent 1d hotspot list, and the prior-ledger gap summary. It reads source but does not write or run targets. Require exactly this JSON:

```json
{
  "missing_units": [
    {
      "entry": "...",
      "invariant": "...",
      "subsystem": "...",
      "bug_class": "...",
      "starting_paths": ["repo/relative/path"],
      "selected_companion_blocks": ["FILE.md#section"],
      "excluded_blocks": [{"block": "FILE.md#section", "reason": "..."}],
      "reason": "source-backed coverage gap"
    }
  ],
  "reassign_ids": ["existing-id-that-did-not-close"],
  "resolved_prior_leads": ["fingerprint"],
  "stop": false
}
```

The critic checks for unmapped entries, unchecked parallel paths, missing lifecycle modes, implied-only invariants without a unit, hotspots no unit reaches, error and cleanup paths no check followed, selected companion classes without a unit, unjustified exclusions, units closed without paths and checks, and prior `needs_validation` or changed-source gaps that no unit addresses. It proposes coverage, not findings. `stop` is the critic's own assessment: `true` only when it accepts no `missing_units` and no `reassign_ids`; the parent's loop condition below, not `stop` alone, decides whether another wave runs. For each fingerprint in `resolved_prior_leads`, the parent marks the linked unit or prior-lead entry resolved and records the critic's source-backed reason.

The parent rejects proposed units outside the review scope or the execution policy, derives canonical IDs for accepted units, and deduplicates them against current units. A prior same-source completed unit may supply evidence; prior `deferred`, `blocked`, `out_of_scope`, or changed-source units become current work and never suppress an accepted unit. Fail rather than merge a canonical ID collision. For each legitimate `reassign_id` with live `blocked`, `covered`, or `candidate` evidence, append that exact terminal record to the unit's `attempts` with the critic's source-backed `reassignment_reason`. Preserve its owner, checks, artifacts, fingerprints, and unresolved facts in that archive. Increment the live `wave`; the next hunter must be a fresh owner and receives an `in_progress` unit with empty live evidence. The hunter's terminal result writes only its new evidence into the live fields. Never copy an archived owner's checks or artifacts into the new live attempt. Sort IDs and validate the ledger before another assignment. In `standard` and `deep`, when the post-wave critic reports no accepted `missing_units` or legitimate `reassign_ids` and no `planned` units remain, spend the separately reserved invocation on a distinct final-clean critic. Complete coverage only when that critic also returns no accepted work. If it finds work, queue it and repeat the wave, post-wave critic, and final-clean process. If time or resources force an early stop, mark every untouched unit `deferred`, preserve the critic's reason, and disclose the gap in the report. Never use a silent wave or agent cap as evidence of complete coverage.

The run profile bounds this loop. A `quick` run has exactly one hunter wave followed by exactly one final critic pass. Add each accepted `missing_unit` to the current ledger and mark it `deferred` with reason `quick_profile_final_critic`. For each legitimate evidence-bearing `reassign_id`, archive the live terminal state in `attempts`, increment `wave`, and set the live state to unassigned `deferred` with empty evidence and reason `quick_profile_final_critic`. Do not launch a second hunter wave or another critic. In a scoped run, the critic still reports out-of-scope gaps it notices, but the parent records them as `out_of_scope` with the critic's reason instead of assigning them. The early-stop rule above is the same mechanism: `quick` is a pre-declared early stop, not evidence of complete coverage.

A budget bounds it the same way. Before assigning each wave, compare remaining budget against its hunter count, the validation reserve, the immediate post-wave critic, and the retained final-clean critic (`quick` reserves only its single final post-wave critic). Shrink the hunter wave to fit, taking units in priority order. If those mandatory reserves do not fit, launch no hunter from that wave and mark its planned units `deferred` with reason `budget_cannot_reserve_critics_and_validation`. Critic-proposed units enter the same ranked queue rather than extending the budget. If surviving candidates exceed the validation reserve, follow the incomplete-run rule in `SKILL.md`: stop hunting, validate in fingerprint order, retain unvalidated units as unresolved candidates, and never present them as findings.
