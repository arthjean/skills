# Data and Persistence Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds durable state: a SQL, document, or key-value database, an ORM or query builder, explicit transactions, schema migrations or backfills, a cache in front of a store, search indexes or materialized views, files used as storage, or persisted serialized formats. A database driver in the manifest is not enough. Select it when source writes or derives persisted state that other code later trusts.

Use `DISTRIBUTED-AND-MESSAGING.md` for consistency across services, events, and outbox relays, `CONCURRENCY-AND-ASYNC.md` for in-process locking, and `NUMERIC-TIME-AND-TEXT.md` for money, decimal, timestamp, and collation semantics. This file owns transactions, constraints, migrations, caches, derived copies, and query semantics.

## Core discipline (include in every agent prompt for this domain)

```
- Establish what the store actually guarantees: isolation level (default and per-transaction overrides), constraints (unique, foreign key, check, NOT NULL), and atomicity of each statement. The code's assumption must match the store, not the shape of the ORM API.
- Prove against the same engine and major version when it is available locally (an embedded engine or a container already present, started for this check). SQLite and in-memory fakes differ in isolation, locking, collation, and type coercion, so a proof on another engine is a `differential` lead, not a confirmation.
- Use scratch databases only, created and destroyed by the check. Never point a check at a shared or deployed database, and never run a migration outside scratch.
- Drive races with two connections and explicit statement ordering (`deterministic_schedule`). Seed migration and decoding fixtures with nulls, duplicates, empty strings, and legacy shapes taken from the history of the schema.
- Use `needs_validation` when the defect depends on production data shape or volume, the deployed isolation level or engine version, or replication lag.
```

## Transaction and isolation bug classes (subagent_type: `general`)

**Lost update in read-modify-write**
Code reads a row, computes in the application, and writes the result back without `SELECT ... FOR UPDATE`, an atomic update expression (`SET balance = balance - $1`), or an optimistic version check. Under READ COMMITTED, the PostgreSQL default, two concurrent writers silently lose one update. ORMs that load an entity, mutate fields, and save it hide this shape. Proof: `deterministic_schedule` with two connections ordered read, read, write, write; control: the atomic update or a version check that rejects the second write.

**Write skew and multi-row checks**
A rule that spans rows ("at least one owner remains", "bookings never overlap", "allocations never exceed the budget") is checked with a SELECT and then enforced by writing a different row. Snapshot isolation (PostgreSQL REPEATABLE READ) lets both transactions commit, breaking the rule. Proof: `deterministic_schedule` running both transactions to their checks before either writes; the fix is SERIALIZABLE with retry on serialization failure, explicit row or advisory locks, or a constraint the database enforces (an exclusion or check constraint).

**Check-then-insert on unique keys**
`if not exists(key) then insert` races: without a unique constraint it creates duplicates, and with one the violation surfaces as an unhandled error instead of a conflict result the caller can act on. A second variant: the application normalizes or compares case differently from the database collation, so the pre-check and the constraint disagree. Proof: `deterministic_schedule` or `stress_loop` with concurrent inserts of the same key; control: an upsert (`ON CONFLICT`, `ON DUPLICATE KEY UPDATE`) or a handled violation mapped to the documented result.

**Transaction scope and connection state leaks**
Work meant to be atomic spans two transactions or autocommit statements; an inner `begin` is silently ignored or becomes a savepoint with different rollback semantics than the author expected; an ORM session is reused after a failed flush; a connection returns to the pool mid-transaction after an exception, so the next borrower inherits open locks or uncommitted state; concurrent tasks share one connection. Proof: `fault_injection` raising between statements and asserting no partial state remains, plus an `integration_test` that borrows the same pooled connection afterward and checks its transaction state.

## Schema and migration bug classes (subagent_type: `general`)

**Migration and code version skew**
During a rolling deploy, old and new code run against one schema: a renamed or dropped column breaks old readers, a new NOT NULL column without a default breaks old writers, a value added to a code enum is rejected by a database check constraint or crashes old readers. Expand-then-contract ordering is missing. Proof: `differential` running the previous release's queries or test suite (checked out from the prior ref into scratch) against the migrated scratch schema, and the new code against the unmigrated one.

**Backfill and data migration correctness**
A batch backfill paginates with OFFSET or a non-unique, mutable column and skips or repeats rows while writes continue; the transform mishandles legacy shapes (null versus empty string, retired enum values, mixed-case keys); a migration that is not idempotent duplicates or corrupts data when rerun after a partial failure; a down migration drops data it cannot restore. Proof: `integration_test` on a scratch database seeded with legacy-shaped fixtures, plus `fault_injection` stopping the migration midway and rerunning it to completion.

**Persisted format evolution**
Serialized blobs, JSON columns, cached objects, or files written by an older version are read by new code: a renamed field silently becomes null or a default, a new required field throws, an unknown enum variant crashes or is dropped, a positional format breaks when fields are reordered. Proof: `unit_test` decoding fixtures in the old format (rebuilt from the type's git history) with the current decoder, and a `property_test` that encodes with the old shape and decodes with the new one.

## Cache and derived data bug classes (subagent_type: `general`)

**Cache-aside invalidation race**
A reader misses, reads the old value from the store, a writer updates the store and deletes the cache key, then the reader writes the old value into the cache, where it stays until the TTL expires. Invalidating before commit lets readers repopulate from pre-commit state; invalidating on only some write paths leaves others stale indefinitely. Proof: `deterministic_schedule` with a gated store read that releases the reader after the writer's delete; control: invalidation after commit plus a versioned or compare-and-set cache write.

**Derived copy drift**
Denormalized counters, search indexes, materialized views, aggregates, and read models are updated on some write paths but not on others (bulk import, admin edit, delete, restore, migration, retry). Proof: `integration_test` that exercises each write path and compares the derived copy with a recomputation, or a model-based `property_test` over random operation sequences asserting that the derived value always equals the recomputed one.

**Status and visibility filters**
Queries forget the soft-delete, archival, draft, or status filter in some places (counts, exports, joins, uniqueness checks that should ignore deleted rows), so deleted items reappear or totals disagree between screens; a unique constraint that includes soft-deleted rows blocks re-creating an item the user deleted. Proof: `integration_test` with a soft-deleted or archived fixture run through every listing, count, and uniqueness path.

## Query semantics bug classes (subagent_type: `general`)

**Null and three-valued logic in queries**
`WHERE status <> 'archived'` excludes rows where status is NULL; `NOT IN (subquery)` returns no rows when the subquery yields a NULL; `COUNT(col)` skips nulls while `COUNT(*)` does not; `SUM` over zero rows returns NULL rather than zero; a WHERE condition on the right table of a LEFT JOIN turns it into an inner join; an ORM translates a filter on `None` differently than the author assumed. Proof: `integration_test` with null-bearing fixtures asserting the exact result set, read alongside the SQL the ORM actually emits.

**Ordering, pagination, and silent limits**
A query without ORDER BY returns engine-dependent order that tests happen to satisfy; ORDER BY a non-unique column with LIMIT and OFFSET yields duplicates and gaps across pages; a cursor built on a mutable or non-unique key loses rows; a default page size or driver limit truncates a result that callers treat as complete. Proof: `integration_test` with tied sort keys and concurrent inserts, plus a `property_test` over page sizes asserting the union of pages equals the full set with no duplicates.

**File storage atomicity and durability**
Files used as a store are rewritten in place instead of written to a temporary file and renamed; the rename happens without flushing the file (and, on POSIX, its directory), so a crash leaves an empty or torn file; two processes append or rewrite the same file without coordination; a failed or short write on a full disk goes undetected. Proof: `fault_injection` that stops the process between steps (a failpoint or a killed child in a harness) and asserts the reader sees either the old or the new content, never a torn mix.

## Universal moves (apply across the above)

- Build a write-path table per entity: every path that writes it (API, batch job, import, admin tool, migration, retry, restore), its transaction boundary and isolation, the constraint that backs each rule, and the derived copies it must update. Gaps between rows are the findings.
- Read the SQL the ORM emits in the scratch database's statement log before reasoning about isolation or nulls. The API shape often hides a second query or a missing lock.
- Reconstruct older data shapes from migration files and the git history of models, then use them as fixtures. Real data is older and messier than the current schema suggests.

## Validation rules (apply before reporting ANY finding here)

1. Name the entity, the rule or invariant, the write or read path that violates it, and the observable wrong state: lost or duplicated row, stale cache value, inconsistent derived copy, wrong result set, or torn file.
2. `confirmed` requires a red check on a scratch store using the same engine semantics, with a control: the serialized schedule, the atomic statement, or the fixture without the triggering shape passes.
3. Check source-visible protection before reporting: a constraint, a lock, a version column, a serializable transaction with retry, or a single-writer architecture. If one covers every real path, reject.
4. A proof on a different engine, isolation level, or major version is a lead. Rerun it on matching semantics or return `needs_validation` naming the engine fact.
5. Return `needs_validation` when the defect depends on production data shape or volume, deployed isolation or engine configuration, or replication lag, and give the scratch fixture or owner-observed query that would decide it.
