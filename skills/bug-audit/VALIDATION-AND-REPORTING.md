# Validation, Structured Output, Verification, and Reporting

### Phase 3: Independently validate every candidate

After the clean coverage-critic pass or an explicitly recorded early stop, consolidate Phase 2 candidates and carried same-source prior confirmations by stable fingerprint and root cause. Give every unique proposed `confirmed` and `needs_validation` candidate to a fresh `general` verifier that did not hunt it. A carried prior confirmation follows the same current verification path even though hunters exclude that unchanged root cause. A verifier may read hunter or prior artifacts but must re-read every cited current source location and independently re-run every decisive check it can run under the execution policy.

Assign each verifier a canonical lowercase unique ID and `<output-dir>/agents/<verifier-id>/scratch/` plus parent-owned `artifacts/`. The verifier builds its own disposable copy in `scratch/` and never writes retained artifacts. It receives only the candidate, its linked coverage-unit checks and artifact paths, architecture facts needed to interpret the path, exact relevant companion validation blocks, the execution policy block from [HUNTING.md](HUNTING.md) with the run's policy filled in, the promotion procedure block below, the `confirmed`, `needs_validation`, and `rejected` branches of `report-schema.json` copied verbatim, and prior records with the same fingerprint. It must not receive another verifier's conclusion.

#### Candidate-verifier prompt

```text
You did not write this candidate. Try to refute it from repository source and your
own bounded checks, run only as the execution policy block in this prompt allows,
inside your own disposable copy. Never edit target source. Write checks and captured
output only in scratch and list the files to promote; the parent promotes them by the
promotion procedure block included verbatim in this prompt. If promotion is
unavailable or fails, do not use that file as evidence.

1. Verify every trace and evidence file, positive line number, scope, and description.
   Confirm the first entry is a real reachable entry (a route, command, job, handler,
   or exported API that something calls outside tests) and the last is the claimed
   failure point.
2. Reconstruct the strongest source-visible guards on the path: types, validation,
   schema constraints, framework guarantees, locks, transactions, retries, and
   supervisors. Check real callers: does any of them deliver the trigger? Where the
   architecture summary names a comparable-stack baseline, note whether the pattern
   matches it, as calibration, never as grounds to dismiss.
3. For a proposed confirmed candidate, re-run the red check yourself. Read the failure
   output: it must fail for the stated reason, not for a harness, setup, or
   environment error. Run the control when one exists. For a non-deterministic proof,
   re-measure runs executed and failed. If the check is wrong or flaky beyond its
   stated rate, rebuild it once; if it still does not go red for the stated reason,
   demote or reject.
4. When the proposed fix is small and local, apply it in your scratch copy and confirm
   the regression test turns green and adjacent tests stay green. A fix that only
   catches, defaults, or guards the symptom is not the root fix: correct the
   remediation. Record what you observed in the confidence reason.
5. Verify that likelihood, impact, confidence, and the remediation match only what the
   evidence establishes.
6. For a proposed needs_validation candidate, decide whether the blocker is genuinely
   outside source and local observation. If source refutes the trace, reject it. If
   you can now produce the proof locally, do so and promote it to confirmed. If the
   missing fact remains decisive, keep needs_validation and make the local and owner
   plans exact and non-destructive.
7. Preserve the fingerprint for the same source-derived root cause across every state.

Return exactly one JSON object and no surrounding prose:
{"decision": "confirmed|needs_validation|rejected", "record": { ... }, "promote": []}
where record exactly matches the decision's verdict branch of the schema included in
this prompt. A corrected record replaces the hunter's wording.
```

Copy this promotion procedure verbatim into every candidate-verifier prompt:

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

A verifier can promote `needs_validation` to `confirmed` only after independently establishing the complete path and a red check for the stated reason. Demote a proposed confirmation to `needs_validation` when a specific production, platform, third-party, or deployment fact remains decisive and unknown. Use `rejected` when source, a re-run check, a visible guard, an unreachable trigger, intended documented behavior, or an impossible prerequisite refutes the claim. A prior confirmed record whose regression test now runs green on current source is `rejected` with reason `fixed_in_current_source`. `needs_validation` is never a parking place for a speculative idea.

The parent checks that each verifier returned the same fingerprint unless it identified a genuinely different root cause. Merge corrections, record the decision in every linked coverage unit, and ensure there is one final record per fingerprint. Discard a malformed or prose-wrapped verifier result without repairing it; re-run that candidate with a fresh verifier when the budget permits, otherwise it remains an unvalidated ledger candidate under the incomplete-run rule.

When verifier evidence updates a ledger check, set that check's `agent_id` to the verifier's canonical ID and list its nonempty repository-relative `reviewed_paths`. Keep the unit-level `reviewed_paths` equal to the union across checks. Use `method: "source"` with `artifact: null` for source-only review. Use `method: "local"` only with a file the parent successfully promoted below `agents/<check.agent_id>/artifacts/`. The unit retains its original assignment owner, so independently owned hunter and verifier checks can coexist. For a carried prior record's seeded `planned` unit there is no prior owner: the verifier that re-checks it becomes the unit's assignment owner, and its re-check is the unit's first check, moving the unit to `candidate` with the carried fingerprint.

If a strict total-agent budget cannot cover every candidate, set the run status to incomplete and follow the deterministic budget rule in `SKILL.md`. An unvalidated candidate remains only in the ledger. It does not enter `findings.json` under any verdict.

### Phase 4: Write and validate `findings.json`

The parent writes all independently decided records to `<output-dir>/findings.json`, sorted by fingerprint. Include:

- `confirmed`: source-grounded defects with a re-run red check, conditions, a regression test, specific remediation, likelihood, impact, overall severity, and confidence.
- `needs_validation`: source-grounded candidates with an exact unresolved blocker and at least one applicable local or owner-observed plan.
- `rejected`: source-grounded candidates disproved during validation, retained so future runs do not repeat the unsupported claim without changed evidence.

Read `report-schema.json` immediately before writing. It uses `additionalProperties: false`; do not carry hunter wrapper fields into a record. Keep these verdict contracts distinct:

- A `confirmed` record uses `bug_class`, `invariant`, `root_cause`, `conditions`, `reproduction`, `regression_test`, `remediation`, `severity`, and `confidence`. It must not use `claimed_root_cause`, `blockers`, `validation_plan`, or `reason`. `reproduction` is target-neutral and uses the target's native interface: function call, test case, CLI invocation, request, message, file fixture, UI action, or local harness as applicable. `observed_result` is nonempty and factual, and `runs.failed` never exceeds `runs.executed`. `regression_test` is self-contained: an owner can add `code` at `file_name` and run `command` without reading the audit.
- A `needs_validation` record uses `bug_class`, `invariant`, `claimed_root_cause`, `trace`, `evidence`, `blockers`, and at least one nonempty `validation_plan.local` or `validation_plan.owner` field. Include both only when both contexts can resolve distinct facts. It must not use severity, reproduction, regression test, remediation, confidence, reason, or a confirmed root cause.
- A `rejected` record uses `bug_class`, `invariant`, `claimed_root_cause`, `trace`, `evidence`, and `reason`. It must not use severity, reproduction, regression test, remediation, confidence, blockers, validation plan, or a confirmed root cause.

Every record has a stable fingerprint, title, description, and repository-relative source paths. A multi-step trace begins with `entrypoint`, ends with `failure`, and uses `propagation` only between them. One-entry traces use `entrypoint` or `failure`. `bug_class` is the exact block reference, such as `BUG-CLASSES.md#State consistency`. Overall severity cannot exceed demonstrated impact.

Run:

```sh
node <skill-dir>/validate-findings.cjs <output-dir>/findings.json
node <skill-dir>/validate-coverage-ledger.cjs <output-dir>/coverage-ledger.json
```

Fix every structural and semantic error before continuing. The findings validator rejects input beyond 5 MiB, 1,000 top-level findings, or 64 nesting levels, and caps reported error output at 100 messages. Validator success proves format and ledger consistency only.

### Phase 5: Verify the final records with fresh eyes

Launch one fresh `general` verifier per final `confirmed` and `needs_validation` record, in parallel. This verifier works from the structured record alone, not from the hunter write-up, and stays inside the execution policy.

In a `quick` run, Phase 3 and Phase 5 merge: the Phase 3 verifier also performs these record checks and returns the final schema-shaped record, so each candidate gets one fresh independent reviewer instead of two. Every other profile keeps the two passes separate. Never skip independent review of a `confirmed` record in any profile.

For `confirmed`, require it to check:

1. Every repository-relative trace and evidence path, line, scope, and described operation.
2. The real entry and the reachability of the trigger from it.
3. Every condition, guard, and source-visible preventing layer.
4. The record's own `regression_test`: add `code` at `file_name` in a fresh disposable copy, run `command`, and confirm it goes red for the stated reason with the stated run counts. A record whose test cannot be run from the record alone is a replacement, not a pass.
5. Severity separation: realistic likelihood, demonstrated impact, overall no greater than impact.
6. Remediation strategy and any `code_changes`, including whether the fix enforces the invariant at its root instead of guarding the symptom.

For `needs_validation`, require it to check:

1. The source path is real and supports only the `claimed_root_cause` stated.
2. Every listed blocker is decisive and not already answerable locally.
3. The candidate names an invariant and a possible concrete wrong result rather than a generic concern.
4. At least one validation-plan field is present and exact. `local` uses a bounded check under an executing policy; `owner` asks an owner to observe a log, metric, data, platform, or configuration fact. Do not invent a plan for an inapplicable context, and never send audit traffic to a deployment.
5. The fingerprint matches prior and current records for the same root cause.

Each verifier returns exactly one JSON object: `{"decision":"verified","fingerprint":"..."}` or `{"decision":"replace","reason":"...","record":{...}}`, with no surrounding prose. A replacement record must match its `confirmed`, `needs_validation`, or `rejected` schema branch. Treat a malformed or prose-wrapped Phase 5 result the same way as in Phase 3: discard it without repairing it and re-run with a fresh verifier when the budget permits.

Do not apply a Phase 5 replacement as final when it promotes a record to a stronger verdict, including any promotion to `confirmed`, or materially changes the root cause, invariant, trace, reproduction input or observed result, regression test, demonstrated impact, or severity. Give that complete replacement to a new independent verifier that did not hunt, perform Phase 3 validation, or propose the Phase 5 replacement. The new verifier rechecks the current source and independently re-runs the decisive check under the execution policy, then returns `verified` or another replacement. Apply a material replacement only after this fresh verification. If another material replacement results, repeat with a fresh verifier. If budget or independence is unavailable, remove the disputed record from `findings.json`, keep its ledger unit as an unresolved candidate, and set `run_status: "incomplete"` with an exact `incomplete_reason`. Non-material wording or repository-line corrections may be applied directly when they do not change meaning or evidence.

After every applied replacement, rerun both validators and update linked ledger decisions. If a final verifier identifies a separate root cause, assign a new fingerprint and send it through independent candidate validation before inclusion. Set `run_status: "complete"` only when every ledger candidate has an independent final disposition and every retained record passes Phase 5.

Do not verify only `confirmed` records. A misleading `needs_validation` handoff wastes owner time and can preserve a false premise.

### Phase 6: Produce target-neutral reports from final records

Only after Phase 5 passes for every record retained in `findings.json`, derive prose from the final records, the ledger, and the hunter `hardening` notes retained in ledger bookkeeping. An incomplete run may report independently verified records, but it must identify each unresolved ledger candidate and must not present it as a finding. The prose files never change a verdict, severity, blocker, or demonstrated impact.

#### `REPORT.md`

Write:

1. Run profile, scope, budget (if set) with agents spent versus planned, source ref, execution policy (with the trust confirmation for `trusted-local`), prior-run use, and explicit deferred and out-of-scope coverage. Name carried same-source confirmations, changed-source revalidations, and prior findings now fixed. A `quick`, scoped, budget-limited, `source-only`, or incomplete run states plainly that it is a partial pass. If candidate validation exhausted a strict budget, state that the run is incomplete and list every unvalidated fingerprint and linked unit; do not describe those candidates as findings. If the budget prevented a mandatory critic, state which critic did not run and make no clean-coverage claim.
2. One short correctness posture summary.
3. A confirmed-findings table: severity, title, bug class, broken invariant, and one-line observed result.
4. Each confirmed finding: repository source location, invariant, trigger, target-native reproduction with run counts, conditions, expected versus actual result, impact, priority rationale, smallest source fix, and where the regression test goes.
5. A separate `NEEDS VALIDATION` table. Give each lead's title, repository trace, exact blocker, bounded local next step, and owner-observed check. Do not assign severity or call it a confirmed defect.
6. A fix queue: confirmed fingerprints ordered by severity, then by shared root cause, each with its regression test command as the reproducer for the fixing step.
7. Separate hardening notes and positive source patterns (invariants that are enforced and tested well).
8. Coverage summary from the ledger: covered, candidate, blocked, and deferred counts, important exclusions, implied-only invariants left unexamined, and the final critic result.

Do not describe rejected records as findings. Mention their fingerprints only when they explain a prior disagreement, a fix since a prior run, or a coverage decision.

#### `FINDINGS-DETAIL.md`

For each confirmed `medium`, `high`, or `critical` record, copy the complete source path and target-neutral reproduction:

- ordered repository-relative trace and evidence;
- the invariant and the trigger;
- native input, invocation, fixture, seed, or schedule and exact bounded instructions;
- red output, run counts, and control output, and the invariant they prove broken;
- conditions and blast radius;
- the regression test code and command;
- source-level remediation.

#### `NEEDS-VALIDATION.md`

For every unresolved record, copy the source trace, verified evidence, invariant, exact blocker, and each applicable local or owner-observed resolution plan. Keep these as prioritized leads without severity. Do not turn them into instructions to probe live systems or assume the missing fact.

A test runner is one possible native interface, not the only one. A library finding may use a function call, a parser a fixture, a CLI a command, a service a request against a loopback instance, a UI a scripted interaction, and a job a direct invocation of its handler. Do not require an endpoint, external account, or live environment that the target does not have.

Keep the report proportional to the evidence. A clean run may have zero confirmed records. State that result and the remaining coverage and validation limits without inventing low-severity findings.
