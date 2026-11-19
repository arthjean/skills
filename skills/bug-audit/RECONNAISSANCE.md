# Reconnaissance

### Phase 1: Map the source and plan coverage

The parent initializes `run-metadata.json`, applies the strict pre-reconnaissance budget gate in `SKILL.md`, then creates agent scratch roots and the shared ledger before hunting. If the gate fails, record the incomplete status in metadata and launch no reconnaissance agent. Reconnaissance reads the target and locally available build and configuration state. Only Agent 1d executes anything, and only under the run's execution policy. No agent contacts deployed endpoints, registries, shared databases, queues, cloud APIs, or other shared services.

Launch the four baseline agents in parallel. Agents 1a to 1c are `research` agents; Agent 1d is a `general` agent when the policy allows execution and a `research` agent under `source-only`. They return structured facts to the parent and do not write outside their own scratch. Agent 1d's prompt carries the execution policy block from [HUNTING.md](HUNTING.md) with the run's policy filled in.

**Agent 1a: Product, stack, and local operation**

```text
Read the target at <target>. Do not use network access. Return:
1. Product type, users, operators, and the critical workflows that must never go wrong
   (money, persisted user data, notifications, scheduling, irreversible actions).
2. Languages, frameworks, runtimes, build system, and the concurrency model
   (threads, async runtime, event loop, workers, processes, scheduled jobs).
3. Repository-relative entry points and subsystem boundaries.
4. Exact offline build and test commands, the test frameworks in use, and which
   property-testing, fuzzing, sanitizer, race-detection, or schedule-exploration tools
   are already installed or vendored. Do not run them.
5. Comparable software or framework conventions visible from local documentation and
   dependencies, with the bug patterns those stacks are known for. If no useful
   comparison is source-grounded, say so.
6. Missing local toolchains or runtime facts that limit bounded execution.
Return only source facts with repository-relative file:line references.
```

**Agent 1b: Invariants and contracts**

```text
Read the source that defines what must stay true. Map:
1. Data invariants: schema constraints, uniqueness, foreign keys, enums, value
   ranges, units, and validation rules, with where each is enforced.
2. State machines: every status or phase field, its allowed transitions, and every
   writer of that field.
3. Resource ownership: each acquire and release pair (connections, files, locks,
   transactions, timers, listeners, subscriptions, tasks, child processes).
4. Error contracts: error types, where each is raised, caught, mapped, logged, or
   retried, and what callers assume when a call returns normally.
5. Concurrency contracts: locks, transactions and their isolation levels, single-writer
   assumptions, idempotency keys, and ordering guarantees.
6. Cross-component assumptions: nullability, units, encodings, time zones, rounding,
   pagination, and versioning at each module, service, or storage boundary.
Classify every invariant by enforcement: enforced (type, constraint, lock, or
transaction), tested, documented, or implied only. Return invariants and their
locations with repository-relative file:line references. Do not infer runtime behavior.
```

**Agent 1c: Entries, flows, and effects**

```text
Inventory every place behavior is driven from outside a module:
- HTTP routes, RPC and message handlers, CLI commands, scheduled and background jobs,
  event and webhook handlers, UI event handlers, exported library functions,
  migrations, startup and shutdown hooks.
For each entry, follow major transformations to its effects: persistent writes,
external calls, outputs, emitted events, and in-memory state shared across calls.
Record parallel paths to the same effect (batch, import, admin, retry, backfill,
migration) and lifecycle modes (startup, shutdown, cancellation, retry, recovery,
upgrade). Return repository-relative paths and line numbers. Be complete, but do not
execute or send inputs.
```

**Agent 1d: Tooling signals and hotspots**

```text
Collect cheap deterministic signals that rank where defects are likely. Under an
executing policy, work only in a disposable copy of the target in your scratch, follow
the execution policy block included in this prompt, and give every command a timeout:
1. Run the type checker or compiler in its strictest configured mode and the configured
   linters. Record warnings and errors with file:line.
2. Run the existing test suite once, only if its setup, fixtures, and environment
   configuration show no reach beyond loopback. Record failures; rerun each failure once
   to separate deterministic failures from flaky tests.
Under every policy:
3. List skipped, disabled, focused, or quarantined tests and what they would cover.
4. From git history (read-only): files ranked by change frequency over the last twelve
   months, files with the most bug-fix commits, and recent reverts.
5. Density of risk signals per file: catch-all handlers, ignored results, unchecked
   assertions, suppressions, TODO/FIXME/HACK comments.
6. Test coverage per module if a configured report already exists or the configured
   command produces one offline.
Return a ranked hotspot list (file or module, signals, counts) and the raw signal
entries. A signal is a lead for hunters, never a finding. Under source-only, read
configuration and committed CI output instead of running anything, and say so.
```

Add focused reconnaissance agents for materially distinct subsystems or runtime modes that these four do not map. Do not silently omit them: if the budget gate in `SKILL.md` blocks a focused agent, launch nothing for it, seed the unmapped area as a `deferred` ledger unit with reason `budget_cannot_reserve_critics_and_validation`, and disclose the gap in the report.

## Prior-run input

Before selecting work, the parent reads every available prior `coverage-ledger.json` and `findings.json` for the same repo:

- Compare the source locations, invariants, conditions, and source-derived identity for every prior record and unit against the current source.
- Carry an unchanged prior `confirmed` record into the current candidate set, with the same fingerprint, only when its relevant source, conditions, and qualifying evidence still apply. Link it to a current `planned` unit with `prior_status: "prior_confirmed_same_source"` and put only that root cause on the hunter exclusion list. The Phase 3 verifier that re-runs the carried record's regression test becomes that unit's assignment owner; its re-check is the unit's first check and moves the unit to `candidate` with the carried fingerprint.
- Build a current planned `prior_confirmed_changed_source` revalidation unit when any relevant source or condition changed. Do not exclude that root cause from hunting or assume the prior verdict still applies.
- Build current work units for every prior `needs_validation`, `deferred`, `blocked`, `out_of_scope`, and changed-source unit. These states are priority input, never deduplication or suppression keys.
- Carry a still-blocked prior `needs_validation` record with the same fingerprint only after current source supports its trace. Link it to a current `planned` unit with `prior_status: "prior_needs_validation"`; the record keeps the unresolved blocker. The Phase 3 verifier that re-checks the carried record becomes that unit's assignment owner; its re-check is the unit's first check and moves the unit to `candidate` with the carried fingerprint. Include the record in final verification.
- Treat prior rejected records as stale claims unless current evidence changes the failed trace or missing condition. An unchanged rejection suppresses only that exact claim, not review of the coverage unit.
- Record missing or incompatible ledgers instead of treating them as empty coverage.

State paths and source refs used in `run-metadata.json`. Summarize only the coverage consequences in `architecture.md`.

## Architecture summary and companion selection

The parent synthesizes `<output-dir>/architecture.md`, with a hard cap of about 1,000 words. Include:

1. Product, critical workflows, and the data and effects that must never go wrong.
2. The comparable-stack baseline from Agent 1a, when one is source-grounded: the bug patterns that stack is known for. Use it to aim hunters, never to dismiss a demonstrated finding.
3. Tech stack, concurrency model, execution policy, and offline build and test limits.
4. Entries and the important entry-to-effect and lifecycle paths.
5. The invariants table: each invariant, its location, and its enforcement level. Invariants that are implied only, or documented but neither enforced nor tested, are the primary hunting ground.
6. The ranked hotspots from Agent 1d with their strongest signals.
7. Repository-relative starting paths.
8. Prior coverage gaps, changed-source and blocked revalidation targets, and same-source confirmed exclusions.
9. A short companion-selection summary derived from [BUG-CLASSES.md](BUG-CLASSES.md): selected files and the source-visible features that require them.

Keep the assignment-level core block, selected companion blocks, and excluded blocks with reasons in each ledger unit, not in `architecture.md`. This keeps the architecture cap valid for large runs and makes the exact hunter prompt map machine-checkable.

Do not select a companion file merely because the language or dependency name appears. Select it because reconnaissance found the feature described by its `When to use this file` section. Do not exclude a visible feature just because another agent will review a related class.

## Deterministic coverage ledger

The parent writes `<output-dir>/coverage-ledger.json` as a top-level JSON array. Derive one unit for every material combination of entry, invariant, subsystem, and applicable core or companion bug class at the granularity the run profile sets (`quick` uses one all-in-scope subsystem identity; `deep` adds lifecycle modes). A combination is material when the entry can reach code that the invariant governs and the class names a way that invariant can break there; do not seed the full cross product. For a scoped run, seed in-scope entries for assignment and retain discovered excluded entries as `out_of_scope` units so later full runs can turn them into current work.

Each dimension has a human label and a stable source-derived value in `canonical_refs`. Use the same canonical reference for the same source object across runs even if its display label changes. Suitable references include a repository-relative entry path plus route, command, or exported name; the source location that defines an invariant (a constraint, a type, a state enum, a lock); a repository package path; and the exact bug-class block reference. A block reference is `FILE.md#` plus the exact class name as written in bold or as a heading in that file: a stable identifier matched against the file text, not a rendered HTML anchor. For companion section blocks, use the heading text before any parenthetical qualifier (for example `Core discipline`). Do not derive references by lowercasing or slugging display labels.

Derive `coverage_id` without lossy slugs:

1. Require every reference to be Unicode NFC with valid scalar values, visible content, no control, format, line/paragraph separator, or default-ignorable code point, and no surrounding whitespace.
2. Encode its UTF-8 bytes with RFC 3986 percent encoding: leave only `A-Z a-z 0-9 - . _ ~` unescaped and use uppercase `%HH` for every other byte.
3. Join encoded `entry`, `invariant`, `subsystem`, and `bug_class` references with `::`; append encoded `lifecycle` when present.

Use the fixed canonical value `profile/quick/all-in-scope-subsystems` for the quick profile's coarsened subsystem dimension. Do not include wave number, agent, verdict, severity, or line number in a reference or ID. Sort units lexicographically by `coverage_id` before each assignment. Fail on every duplicate ID. If duplicate IDs have different semantic fields, treat that as a canonical identity collision; never merge or silently overwrite them. The validator also rejects one semantic tuple represented by different canonical references.

Each unit records:

```json
{
  "coverage_id": "...",
  "canonical_refs": {
    "entry": "src/api/orders.ts#POST /orders/:id/cancel",
    "invariant": "src/orders/state.ts#OrderStatus transitions",
    "subsystem": "packages/api",
    "bug_class": "BUG-CLASSES.md#State consistency"
  },
  "entry": "...",
  "invariant": "...",
  "subsystem": "...",
  "bug_class": "...",
  "starting_paths": ["repo/relative/path"],
  "core_bug_class_block": "BUG-CLASSES.md#State consistency",
  "selected_companion_blocks": ["FILE.md#section"],
  "excluded_blocks": [{"block": "FILE.md#section", "reason": "..."}],
  "prior_status": "new|prior_confirmed_same_source|prior_confirmed_changed_source|prior_needs_validation|prior_deferred|prior_blocked|prior_out_of_scope|prior_covered_same_source|prior_covered_changed_source|prior_rejected_claim_changed|none",
  "attempts": [],
  "wave": 1,
  "status": "planned",
  "agent_id": null,
  "reviewed_paths": [],
  "local_checks": [],
  "result_fingerprints": [],
  "unresolved": []
}
```

When `lifecycle` is material, add both `canonical_refs.lifecycle` and a human `lifecycle` field. `core_bug_class_block` is null only when no core block applies. The selected companion list includes each applicable class plus its companion `Core discipline`, `Universal moves`, and `Validation rules`; `excluded_blocks` records every considered but unselected block and the source fact that excludes it.

The parent may add bookkeeping fields, such as the hotspot rank that drives assignment order, but keeps the semantic fields above stable. In `prior_status`, `new` marks an entry first seen in this run when compatible prior ledgers exist; `none` marks a unit seeded when no compatible prior ledger is available. Prior `deferred`, `blocked`, `out_of_scope`, and changed-source units initialize as current `planned` work when now in scope. A prior same-source covered unit remains visible in the current ledger; assign changed source, hotspots, and exact conflicts first, then use the coverage critic to decide whether it needs another pass.

`attempts` is an append-only archive for evidence-bearing assignments that a coverage critic reopens. Before reassignment, append the prior unit's exact `wave`, `status`, `agent_id`, `reviewed_paths`, `local_checks`, `result_fingerprints`, and `unresolved`, plus the critic's source-backed `reassignment_reason`. Only `blocked`, `covered`, and `candidate` states can be archived. Archived attempts retain the same state and evidence invariants as live units, use strictly increasing waves below the current wave, and retain their producing owners and artifacts. The next assignment increments `wave`, uses a fresh owner, and starts with empty live evidence. If the profile or budget prevents another assignment, increment `wave` and use live `deferred` state with null owner, empty evidence, and the stop reason. Never copy an archived owner's checks or artifacts into the live state. A later live terminal state contains only the new attempt's evidence; the archive remains unchanged.

Enforce this state table exactly:

| Status | Unit `agent_id` | `reviewed_paths` / `local_checks` | `result_fingerprints` | `unresolved` |
|---|---|---|---|---|
| `planned` | null | empty | empty | empty |
| `not_applicable`, `out_of_scope`, `deferred` | null | empty | empty | nonempty reason |
| `in_progress` | canonical owner | empty | empty | empty |
| `blocked` | canonical owner | both nonempty owned partial evidence | empty | nonempty blocker |
| `covered` | canonical owner | both nonempty | empty | empty |
| `candidate` | canonical owner | both nonempty | nonempty | optional |

Canonical agent IDs match `^[a-z0-9][a-z0-9_-]{0,63}$` and are not Windows device names. Lowercase is mandatory, so one ledger cannot contain case-fold aliases. The unit `agent_id` records the assignment owner. Every check records its own `agent_id` and nonempty `reviewed_paths`; the unit-level `reviewed_paths` is exactly their union. A source-only check uses `artifact: null`. A local check requires a regular file promoted only by the parent under exactly `agents/<check.agent_id>/artifacts/`. This lets hunter and verifier checks coexist in one unit. Scratch paths, output-root files, symlinks, special files, and another check owner's artifacts are not evidence.

The ledger is the coverage claim. An architecture summary, agent count, or generic "error handling reviewed" sentence is not coverage evidence. Phase 2 closes units only from the paths and checks in a hunter's structured result.

Run `node <skill-dir>/validate-coverage-ledger.cjs <output-dir>/coverage-ledger.json` after seeding, after every parent update, and before Phase 6. The validator rejects input beyond 5 MiB, 64 nesting levels, 10,000 units, 1,000 entries in a nested collection, or 500,000 traversed values, and caps reported validation errors at 100. In practice the 5 MiB byte limit holds roughly 2,000 to 5,000 realistic units, so it binds before the 10,000-unit cap. Fix every error before assigning work or making a coverage claim.
