# Concurrency and Async Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds work that runs concurrently inside one process: threads or thread pools, an async runtime (the JavaScript event loop, `tokio`, Python `asyncio`, Kotlin coroutines, .NET tasks), goroutines, actors, worker pools, timers, background tasks, or shared mutable state reached by more than one execution. An async dependency alone is not a reason to select it. Select it when source shows two executions that can touch the same state, a suspension point inside a check-then-act sequence, or work that can outlive its caller.

Use `DISTRIBUTED-AND-MESSAGING.md` when the concurrent parties are separate processes or services, `DATA-AND-PERSISTENCE.md` for transaction isolation and database races, and `FRONTEND-AND-UI-STATE.md` for out-of-order UI requests and effect cleanup. This file owns in-process scheduling, synchronization, cancellation, and task lifetime.

## Core discipline (include in every agent prompt for this domain)

```
- Name the executions involved, the shared state or resource they touch, and every suspension point between a check and the act that relies on it: `await`, `yield`, callback, lock release, channel operation, or thread preemption.
- An interleaving is a hypothesis until a schedule shows it. Force it with a deterministic schedule (barriers, latches, gated fakes, injected yields, a paused or fake clock, a single-threaded test executor, `loom` for Rust), or measure a reproduction rate with a bounded stress loop and report runs executed and failed.
- Pair every red run with a control: the same check passes when the operations are serialized or the suspension point is removed. A failure that persists when serialized is not a concurrency defect.
- Use the toolchain's race detection where present (`go test -race`, ThreadSanitizer, Miri for Rust `unsafe`) as a lead, then show the wrong observable result it causes.
- Use `needs_validation` when the defect depends on production concurrency levels, real hardware scheduling, or runtime settings (worker counts, pool sizes) that source does not fix.
```

## Async control-flow bug classes (subagent_type: `general`)

**Unawaited or detached async work**
A promise, future, or task is created but never awaited, joined, or supervised: `array.forEach(async ...)` (the returned promises are ignored), a missing `await` on a write before returning success, `asyncio.create_task` without a retained reference (the event loop keeps only a weak reference, so the task can disappear mid-execution), a Rust future constructed but never polled, a `CompletableFuture` whose exception nobody observes. Effects: success reported before the work finishes, errors lost, work running after the request ended. Proof: `unit_test` asserting the effect is visible when the function returns, or `fault_injection` failing the inner operation and asserting the caller observes the failure.

**Partial failure in fan-out**
`Promise.all` rejects on the first rejection while the sibling promises keep running and their side effects still land; `asyncio.gather` without `return_exceptions` propagates the first exception without canceling the other awaitables; a Go error group or hand-rolled `WaitGroup` returns the first error but leaves goroutines writing into shared slices. Check what the caller believes after the first failure, whether siblings commit effects after the caller returned, and whether a retry then repeats them. Proof: `fault_injection` failing one branch, asserting the final state and that no sibling effect lands after the caller has returned.

**Cancellation safety**
Dropping or canceling a future at a suspension point leaves work half done. In `tokio::select!`, losing branches are dropped, so a branch that had already consumed data through a method that is not cancel safe (a partially read frame, a buffered line) loses it; the tokio docs state which methods are cancel safe. Python's `CancelledError` derives from `BaseException`, so a bare `except:` or `except BaseException:` swallows cancellation and the task keeps running; Kotlin code that catches `CancellationException` does the same. An `AbortSignal` accepted but never passed downstream lets work continue. Check the cleanup on the cancel path and the state left between two writes. Proof: `deterministic_schedule` that cancels at each suspension point in turn (or fires a controlled timeout between steps) and asserts the invariant after every cancellation.

**Re-entrancy across suspension points**
A function checks state, awaits, then acts on the stale check: `if (!cache.has(k)) { const v = await load(k); cache.set(k, v) }` loads twice, and the slower, older result overwrites the newer one; a lazy singleton whose initializer awaits lets a second caller start a second initialization; an event handler re-enters itself through a callback it triggers; a Rust `RefCell` borrowed across an `.await` on a single-threaded executor panics when another task borrows it. Proof: `deterministic_schedule` with two calls and a gated dependency that releases them in the bad order; control: the same calls run sequentially.

## Synchronization bug classes (subagent_type: `general`)

**Lock held across a suspension or blocking call**
A `std::sync::Mutex` guard held across `.await` blocks the executor thread and can deadlock when the task that would release it is scheduled on that same thread (clippy's `await_holding_lock` lint flags this shape). Holding any lock while calling unknown callbacks, logging sinks, or blocking I/O stretches the critical section. In Rust, a temporary guard in a `match` scrutinee (and, depending on edition, an `if let` scrutinee) lives for the whole block. Blocking calls inside async code (`std::thread::sleep`, synchronous file or network I/O, CPU-heavy loops, `time.sleep` under `asyncio`) stall every task sharing the thread. Proof: `deterministic_schedule` or `harness` on a single-worker runtime showing the second task makes no progress within a timeout; control: the guard dropped before the suspension point.

**Lock ordering and self-deadlock**
Two paths take the same locks in different orders; a non-reentrant lock is re-acquired through a callback or recursion; a thread joins itself or blocks on a future only it can complete; a bounded channel's sender waits on a receiver that is itself waiting on the sender. Build the lock-acquisition graph from source and look for cycles, including locks taken inside library callbacks. Proof: `deterministic_schedule` with barriers that park each thread after its first acquisition, asserting a timeout; `sanitizer` (ThreadSanitizer reports lock-order inversions) where the toolchain supports it.

**Unsynchronized shared mutation and publication**
Data races and non-atomic read-modify-write: `counter += 1` from several threads, check-then-act on a concurrent map (`if !map.contains(k) { map.insert(k, v) }`), a Java `HashMap` mutated from several threads, iteration while another thread mutates (Java's `ConcurrentModificationException`, Go's fatal concurrent map writes), double-checked lazy initialization without safe publication (`volatile`, `OnceLock`, `sync.Once`), a Go `WaitGroup.Add` called inside the goroutine instead of before starting it, atomics with an ordering too weak to publish data (`Relaxed` used as a release). Proof: `sanitizer` (`go test -race`, ThreadSanitizer) plus a `stress_loop` counting lost updates, or `deterministic_schedule` via `loom` for Rust atomics; control: the lock or correct ordering restored.

## Task and resource lifetime bug classes (subagent_type: `general`)

**Leaked tasks, timers, and listeners**
Background work outlives its owner: a goroutine blocked forever sending on a channel nobody reads, a `setInterval` never cleared, an event listener registered per request, a spawned task with no join handle or shutdown signal, a `ThreadLocal` in a pooled thread retaining request data, a subscription never disposed. Effects: memory growth, work after shutdown, handlers firing several times. Proof: `leak_measurement` over N iterations counting live goroutines (`runtime.NumGoroutine`), active handles, listener counts, or tasks, showing monotonic growth after cleanup should have run; control: the same loop with the cleanup in place.

**Shutdown and drain ordering**
Graceful shutdown closes the pool, database client, or channel before in-flight tasks finish, or lets tasks continue after the resources they need are gone; the process exits (`process.exit`, `std::process::exit`, `os._exit`) before buffered writes, logs, or queued tasks flush; two signals start shutdown twice. Proof: `harness` that starts work, triggers shutdown mid-flight, and asserts each unit of work either completes or reports failure, never silently vanishes.

**Backpressure and unbounded queues**
An unbounded channel, task queue, or buffer absorbs work faster than consumers drain it, so memory and latency grow without limit; a bounded queue that drops on full without accounting loses work silently; a producer that ignores a write's backpressure signal (`write()` returning `false` on a Node stream) buffers everything in memory. Proof: `stress_loop` or `harness` with a deliberately slowed consumer measuring queue depth and memory, and asserting that every dropped item is counted or reported.

## Universal moves (apply across the above)

- Build a suspension table per shared resource: the executions that touch it, its guard, every suspension point inside the guarded region, cancellation behavior, and the owner of cleanup. Most concurrency defects are one row with a suspension point between a check and the act that depends on it.
- List every place that spawns work (`spawn`, `go`, `create_task`, `new Thread`, `setTimeout`, executor submission) and name who awaits it, who cancels it, and where its errors go.
- Make time and scheduling controllable before testing: fake or paused clocks, single-threaded executors, gates injected at the suspension point, seeded schedulers. Record every seed and schedule so a verifier can replay it.

## Validation rules (apply before reporting ANY finding here)

1. Name the executions, the shared state or resource, the exact suspension or preemption point, and the observable wrong result: lost update, duplicate effect, stale overwrite, deadlock within a timeout, or leaked task.
2. `confirmed` requires a red check: a deterministic schedule that fails on every run, or a stress loop with stated runs executed and failed, plus a control that passes when the operations are serialized or the guard is fixed.
3. A race detector report, sanitizer warning, or lint hit is supporting evidence only. Show the consequence it produces in this code.
4. Look for source-visible serialization that prevents the interleaving in real use: a single-threaded runtime, an actor or queue that serializes access, a framework-level lock, a per-key mutex. If it holds on every real path, reject the candidate.
5. Return `needs_validation` when the defect requires production concurrency, real thread counts, hardware memory-model behavior, or runtime configuration absent from source, and give the bounded local schedule or owner-observed metric that would settle it.
