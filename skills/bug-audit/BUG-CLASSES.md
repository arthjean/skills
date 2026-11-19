# Bug Classes

#### Bug classes: choose and split based on Phase 1

Select bug classes relevant to the target. Every core class applies to most codebases, but its weight does not: a stateless parser lives on edge cases, a job runner on errors and concurrency, a long-running service on resource lifecycle. Add target-specific classes from Phase 1 and split large codebases per subsystem. Frame work as finding, proving, fixing, and prioritizing defects. Keep proof to source review and bounded local checks under the run's execution policy.

Use `confirmed` only when source evidence and a red check establish the invariant, the trigger, and the wrong result. Use `needs_validation` when a specific production, platform, third-party, or deployment fact is unavailable; state the missing fact and the safe owner-observed or local check that resolves it.

> **Concurrent and async targets** (threads, async runtimes, event loops, workers, actors, locks, atomics, channels, timers, cancellation): use [CONCURRENCY-AND-ASYNC.md](CONCURRENCY-AND-ASYNC.md).
>
> **Data and persistence targets** (databases, ORMs, transactions, migrations and backfills, caches, search indexes, file storage, serialization formats and schema evolution): use [DATA-AND-PERSISTENCE.md](DATA-AND-PERSISTENCE.md).
>
> **Distributed and messaging targets** (multiple services, queues, brokers, webhooks, scheduled jobs, retries, sagas, outboxes, cross-service calls): use [DISTRIBUTED-AND-MESSAGING.md](DISTRIBUTED-AND-MESSAGING.md).
>
> **Frontend and UI-state targets** (SPAs, component frameworks, client stores, effects, routing, forms, server rendering and hydration, client persistence): use [FRONTEND-AND-UI-STATE.md](FRONTEND-AND-UI-STATE.md).
>
> **Native and systems targets** (Rust, C, C++, Go, Zig, `unsafe`, FFI, manual memory or handle management, OS interfaces, cross-platform builds): use [NATIVE-AND-SYSTEMS.md](NATIVE-AND-SYSTEMS.md).
>
> **Numeric, time, and text-heavy targets** (money, quantities, units, rounding, dates, time zones, durations, scheduling, Unicode text, locale-dependent parsing or formatting): use [NUMERIC-TIME-AND-TEXT.md](NUMERIC-TIME-AND-TEXT.md).
>
> **CLI, configuration, and environment targets** (command-line tools, config files and precedence, environment variables, file system operations, process lifecycle and signals, cross-platform paths): use [CLI-CONFIG-AND-ENVIRONMENT.md](CLI-CONFIG-AND-ENVIRONMENT.md).
>
> **AI and LLM integration targets** (model calls, structured output parsing, streaming, tool or function calling, prompt assembly, retrieval, token budgets): use [AI-AND-LLM-INTEGRATION.md](AI-AND-LLM-INTEGRATION.md).

**Logic and specification** (subagent_type: `general`)
The code runs without error and returns the wrong answer. Compare each computation with the invariant it serves, not with what the code appears to intend:
- Inverted or incomplete conditions: `&&` for `||`, a negation lost in a refactor, a missing branch in a chain of `if`/`else if`, a `switch` or `match` whose default silently absorbs a new case.
- Off-by-one: inclusive versus exclusive bounds, `<` for `<=`, fencepost errors in pagination, slicing, ranges, retries, and loop termination.
- Wrong operand: the right formula on the wrong variable, a shadowed name, a stale copy instead of the updated value, a field from the wrong object of the same type.
- Units and dimensions: milliseconds against seconds, cents against units, bytes against characters, percentages as 0 to 1 against 0 to 100.
- Order of operations: filter before sort versus sort before filter, normalize before compare, deduplicate before count, apply discount before or after tax as the rule states.
- Specification drift: a comment, docstring, test name, README, or API contract that says one thing while the code does another. The defect is real when a caller relies on the documented behavior.

**Edge cases and boundary values** (subagent_type: `general`)
The happy path works and the boundary breaks. For every accepted input, collection, and state, try the values the interface admits but the author did not picture:
- Empty, single, and maximum: empty string, empty list, one element, exactly at a limit, one past it, the largest representable value.
- Zero, negative, and special numbers: zero divisors, negative counts or durations, `NaN`, infinities, negative zero, very small and very large magnitudes.
- Duplicates and ordering: repeated keys, ties in sorting, unsorted input to code that assumes order, the same item submitted twice.
- Text beyond ASCII: combining marks, emoji, right-to-left text, very long strings, whitespace-only values, mixed case, leading zeros.
- Time boundaries: midnight, month and year ends, leap days, DST transitions, expiry exactly at the boundary instant.
- First and last runs: first-ever execution with no prior state, the last item of a batch, an empty page after the final page.

**Null, absent, and unexpected values** (subagent_type: `general`)
A value arrives in a shape the code does not handle. Trace where each value can originate, not only where it is used:
- Null, undefined, `None`, or `nil` returned by lookups, optional fields, failed parses, or missing relations, then dereferenced or treated as a real value.
- Missing keys and partial objects: absent JSON fields, sparse arrays, records created by an older version, objects built field by field and observed half-built.
- Type confusion: numeric strings, booleans as strings, a single value where a list is expected, integer versus float, coercion in loose equality or concatenation.
- Unchecked assertions: non-null assertions, `unwrap`, `expect`, forced casts, `as` conversions, or `!` on values whose presence no invariant guarantees.
- Schema drift: data written under an old schema read by new code, enum values added on one side only, renamed fields with a silent default.
- Sentinels: `-1`, empty string, zero dates, or magic values that collide with real values or leak into calculations.

**Error handling and propagation** (subagent_type: `general`)
A failure happens and the system misreports it. Follow every error a callee can produce to where it ends up and what the caller then believes:
- Swallowed errors: empty `catch`, `except: pass`, ignored `Result` or return code, `.catch(() => {})`, a log line that replaces handling, a discarded error from `close`, `flush`, or `commit`.
- False success: a function that returns a default, empty list, or `true` after a failure, so the caller acts as if the operation happened.
- Misrouted errors: a broad catch that turns a programming error into "not found", the wrong HTTP status or error code, lost cause chains, an error attached to the wrong request or user.
- Partial failure: step two of three fails and step one is neither rolled back nor recorded, leaving work half-applied.
- Retry semantics: retrying non-idempotent operations, retrying on errors that can never succeed, missing backoff ceilings, retries that hide the first error.
- Async failure paths: unawaited promises, unhandled rejections, errors thrown inside callbacks or event handlers that no caller can observe, background tasks whose failures vanish.
- Cleanup on failure: `finally` blocks that throw and mask the original error, cleanup skipped by an early return or exception.

**State consistency** (subagent_type: `general`)
Two pieces of state that must agree stop agreeing. Name each pair, the rule that binds them, and every writer:
- Multi-step updates that are not atomic: two writes that must both happen, separated by a point where failure or a crash leaves one without the other.
- Derived and cached state: counters, totals, denormalized fields, caches, search indexes, and in-memory mirrors that a write path forgets to update or invalidate.
- State machines: transitions the model forbids but the code allows, skipped steps, replays of completed flows, terminal states that can be left, missing transition guards on alternate paths (admin, import, retry).
- Idempotency: an operation applied twice yields a different state than once, through double submission, redelivery, retry, or replay.
- Ownership and lifecycle: soft-deleted, archived, or expired records still reachable through one path; orphans left when a parent is deleted.
- Rollback and recovery: undo, restore, or compensation that applies old rules, misses derived state, or restores a state the current invariants forbid.

**Concurrency and ordering** (subagent_type: `general`)
Two executions interleave and break an invariant that holds for each alone. For every shared value, find the guard and every point where control can switch:
- Check-then-act: an existence, balance, or permission check followed by an action in a separate step, with no lock, transaction, or conditional write binding them.
- Lost updates: read-modify-write on shared rows, files, caches, or in-memory maps without compare-and-set, version checks, or serialization.
- Interleaving points: every `await`, yield, callback, lock release, and message send is a point where another execution can change shared state.
- Ordering assumptions: events, messages, responses, or file writes assumed to arrive or complete in issue order.
- Deadlocks and starvation: inconsistent lock order, locks held across blocking calls or awaits, a pool exhausted by tasks waiting on each other.
- Re-entrancy: a callback, event handler, or recursive path that re-enters code mid-update and observes or mutates half-updated state.
- Shared mutable state hidden in module globals, singletons, static caches, or default arguments reused across calls or requests.

**Resource lifecycle** (subagent_type: `general`)
Something acquired is not released on every path. For each acquire, find the release on the success path, every error path, every early return, and cancellation:
- Handles: files, sockets, database connections, cursors, transactions, locks, and child processes left open by an exception or early return.
- Registrations: event listeners, observers, subscriptions, signal handlers, intervals, and timers registered repeatedly or never removed.
- Tasks and threads: background tasks, goroutines, promises, or threads started without an owner that awaits, cancels, or bounds them.
- Growth without eviction: caches, maps keyed by request or user, memoization tables, queues, and logs with no size, age, or count bound.
- Temporary artifacts: temp files, directories, staging rows, and partial uploads not cleaned up after failure.
- Pool accounting: checkouts without check-ins on error paths, slots retained past a timeout, connection leaks that surface only under load.

**Cross-component contracts** (subagent_type: `general`)
Each component is correct alone, and the pair is wrong. Compare the exact guarantee one side produces with the exact assumption the other side makes:
- Nullability, units, encodings, rounding, and time zones across a function, module, service, or storage boundary.
- Pagination, sorting, and filtering contracts: a client assuming stable order the server does not guarantee, a cursor invalidated by concurrent writes.
- Error contracts: a callee that throws where callers expect a return value, or returns an error value callers never inspect.
- Versioned contracts: client and server, producer and consumer, writer and reader at different versions during a rollout or after a partial upgrade.
- Truncation and limits: a field truncated, rounded, or clamped by one layer while another layer relies on the full value.
- Second-order use: a value valid where stored and invalid where later used, such as a display name reused as a key or a local time stored as UTC.

**Wildcard** (subagent_type: `general`)
You are not given a category. Find defects outside the standard classes already assigned.

Read code that looks boring, old, or disconnected from the main flows. Follow incomplete, experimental, compatibility, and fallback features, but keep the same invariant, trigger, and proof requirements as every other class.

Use these starting points, but do not limit yourself to them:
- What is the strangest code in the codebase? Why does it exist, and what breaks if its assumption stops holding?
- Which features look half-finished, experimental, or bolted on? They had the least review.
- Which functions have no test, or only tests of the happy path? Compare the cases the tests cover with the cases the interface accepts.
- Which fixes in git history were reverted, partially applied, or patched only at one call site of a repeated pattern?
- What happens when features that were not designed together combine: import plus retry, localization plus caching, pagination plus concurrent deletes?
- What does the code assume about its environment: a single instance, an accurate clock, a case-sensitive file system, a POSIX shell, a UTC server zone?
- Where does a comment explain why something is safe? Verify the explanation against the current code.
- Where are variables named `temp`, `hack`, `legacy`, `old`, or `workaround`? Read them closely.

Pursue anomalies inside your assigned scope until the invariant is settled. If something looks strange, read it until you can state whether it is correct.

**Obvious things** (subagent_type: `general`)
Other agents hunt subtle defects. This agent checks the basic signals that are easy to overlook because everyone assumes someone else already checked them:
- `TODO`, `FIXME`, `HACK`, `XXX`, and `BUG` comments describing a known defect or missing case.
- Empty or catch-all error handlers, ignored return values, and discarded `Result` or error values.
- `unwrap`, `expect`, non-null assertions, forced casts, and `any` or untyped escapes on values from outside the module.
- Floating promises and unawaited async calls, `async` callbacks passed to synchronous iteration (`forEach`, `map` without `Promise.all`).
- Disabled, skipped, focused, or quarantined tests (`skip`, `only`, `xit`, `@Ignore`, `#[ignore]`) and the behavior they no longer cover.
- Compiler, type checker, and linter warnings that the build tolerates, and suppressions (`eslint-disable`, `@ts-ignore`, `#[allow(...)]`, `# type: ignore`, `nolint`) near logic.
- Loose equality, implicit coercion, mutable default arguments, integer division where a fraction is expected, and copy-pasted blocks that differ by one token.
- Hard-coded dates, years, time zones, limits, and environment-specific paths.
- Dead branches and unreachable code that a refactor left behind, and duplicated logic where one copy was fixed and the other was not.

This agent does not need to be creative. It needs to be thorough and literal. Check every item and report each result.

**Important**: For any candidate this agent reports, it must trace the full path, not only the surface signal. An empty catch is a finding only if a reachable failure lands in it and someone acts on the resulting state; a `TODO` is a finding only if the missing case is reachable. A signal is not a finding: trace the wrong result and prove it before reporting.
