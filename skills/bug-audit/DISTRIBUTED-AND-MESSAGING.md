# Distributed Systems and Messaging Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds separate processes or services that coordinate: HTTP or RPC clients with retries, message queues and brokers (Kafka, RabbitMQ, SQS, NATS, Redis streams), incoming or outgoing webhooks, scheduled jobs that run on several instances, distributed locks or leases, sagas, event sourcing, or an outbox relay. A client library in the manifest is not enough. Select it when source performs an effect whose outcome another process observes, retries, or replays.

Use `CONCURRENCY-AND-ASYNC.md` for races inside one process, `DATA-AND-PERSISTENCE.md` for single-database transactions and caches, and `AI-AND-LLM-INTEGRATION.md` for model API retries and streaming. This file owns delivery semantics, ordering, crash points between effects, time across hosts, and version skew between services.

## Core discipline (include in every agent prompt for this domain)

```
- State the delivery and failure semantics the code relies on (at most once, at least once, ordered per key, exactly once) and the semantics the transport and code actually provide. Every remote call has three outcomes: success, failure, and unknown (a timeout after the remote side may have acted).
- For every pair of effects (database write, publish, external call, acknowledgment), ask what state remains if the process dies between them and what the retry or redelivery then does.
- Prove with an adversarial fake at the transport seam: an in-memory broker or fake client that duplicates, reorders, delays, drops, or times out after applying the effect, driven by a seeded schedule (`fault_injection`, `deterministic_schedule`). Use a local broker only if it is already present and started for this check.
- Never send messages, webhooks, or calls to deployed services, shared brokers, or provider sandboxes tied to real accounts.
- Use `needs_validation` when the defect depends on broker or provider configuration (retention, redelivery policy, partition count, visibility timeout), deployed instance counts, or real network behavior.
```

## Delivery and idempotency bug classes (subagent_type: `general`)

**Non-idempotent consumer under redelivery**
At-least-once delivery (an SQS message whose visibility timeout expires, a Kafka consumer that restarts before committing its offset, a RabbitMQ requeue after a lost connection, a webhook provider retrying on a slow response) hands a handler a message whose effects it already applied: a second charge, a duplicate email, a counter incremented twice. Check the deduplication key, where it is stored, and whether the dedup check and the effect commit atomically. Proof: `fault_injection` delivering the same message twice, including a crash after the effect but before the acknowledgment, asserting exactly one effect.

**Retry with ambiguous outcomes**
A client retries a non-idempotent request after a timeout when the server already completed it; there is no idempotency key, or the key is generated per attempt instead of per logical operation; retries stack across layers (SDK, application, gateway) and multiply attempts; non-retryable errors (validation, conflict) are retried in a loop. Proof: `fault_injection` with a fake server that applies the effect and then times out, asserting a single effect and a correct final status reported to the caller.

**Acknowledgment and offset ordering**
The consumer acknowledges or commits before processing, so a crash loses the message; background auto-commit advances offsets for messages still in flight; messages from one partition are processed concurrently and the highest offset is committed while a lower one failed, skipping it permanently; a message that always fails is redelivered forever and blocks its partition or queue. Proof: `harness` with a fake consumer that crashes at each point in the receive, process, commit sequence, asserting no message is lost and none is skipped.

## Ordering and consistency bug classes (subagent_type: `general`)

**Out-of-order events**
A handler assumes events for one entity arrive in order (created before updated, version N before N+1), but consumer concurrency, multiple partitions, retries, or a webhook provider reorder them; last-write-wins then applies a stale event over newer state, or an update for an unknown entity is dropped. Proof: `deterministic_schedule` delivering events in a permuted order, and a `property_test` over permutations asserting the final state equals the in-order result; control: a version or sequence check that discards stale events.

**Dual-write and outbox gaps**
Code writes the database and then publishes an event, so a crash between the two loses the event; or it publishes and then commits, so a rollback leaves a phantom event; an outbox relay marks rows sent before the broker confirms, or two relay instances process the same rows without claiming them. Proof: `fault_injection` crashing between the two effects at each position, asserting that the system converges to one consistent outcome after restart.

**Read-your-writes and replica lag**
Code writes to the primary and immediately reads from a replica, cache, or search index that has not caught up, then makes a decision on the stale read (validation, redirect, rendered confirmation, follow-up write). Proof: `harness` with a fake replica that lags behind the primary by a controlled delay; real lag magnitudes usually require `needs_validation` with an owner-observed replication metric.

## Time and coordination bug classes (subagent_type: `general`)

**Clock skew and cross-host time comparison**
Code compares timestamps produced on different hosts (event ordering, last-write-wins, expiry, lease checks, `updated_at > last_sync`), or measures durations with the wall clock, which jumps under NTP adjustment, instead of a monotonic clock. Proof: `unit_test` with injected clocks offset by a realistic skew, and a `property_test` over skews and adjustments asserting ordering and expiry decisions stay correct.

**Lease or lock expiry during work**
A distributed lock (a Redis key with a TTL, a database lease, a coordination-service lock) expires while its holder is paused or slow; another instance acquires it and both act; the protected resource checks no fencing token, so the stale holder's late writes win. Proof: `deterministic_schedule` with a fake clock advanced past the TTL mid-work, asserting the resource rejects the stale holder's writes.

**Scheduled jobs across instances**
A cron-style job runs on every replica instead of one; leader election skips a run during failover; a job missed during downtime is either never caught up or runs every missed occurrence at once; a long run overlaps its next trigger. Proof: `harness` with two scheduler instances on a shared fake clock, asserting each occurrence runs exactly once and the overlap policy holds.

## Version and contract bug classes (subagent_type: `general`)

**Service version skew**
During rolling deploys, or with independently deployed clients, producers and consumers run different versions: a field the new consumer requires is missing from old producers' messages, an enum value unknown to the old consumer crashes it or is silently dropped, an existing field changes meaning or units without a new name. Proof: `differential` feeding messages produced by the previous version's code (checked out from the prior ref into scratch) to the current consumer and the reverse, or a `unit_test` with captured fixtures of both shapes.

**Timeout and deadline mismatch**
A client timeout shorter than the server's normal work makes the client retry while the server is still processing, duplicating effects and load; a deadline is not propagated, so downstream work continues after the caller gave up; some calls have no timeout at all and hang a worker indefinitely. Proof: `fault_injection` with a slow fake dependency and a `harness` asserting that canceling the caller stops downstream work and that no call blocks past its stated budget.

## Universal moves (apply across the above)

- Build an effects timeline per operation: the ordered effects, every crash point between them, the retry owner at each point, the deduplication key, and where acknowledgment happens. Check each crash point for loss, duplication, and divergence.
- Drive tests through an adversarial fake transport with seeded schedules that duplicate, drop, delay, reorder, and time out after success. A happy-path broker test proves nothing about delivery semantics.
- Diff producer and consumer schemas across git refs to find fields and enum values that changed while both versions can be live.

## Validation rules (apply before reporting ANY finding here)

1. Name the operation, the delivery or ordering assumption it relies on, the crash point, duplicate, reorder, or delay that breaks it, and the observable wrong state: lost effect, duplicated effect, stale overwrite, divergent replicas, or a hung worker.
2. `confirmed` requires a red check through a fake transport or local broker started for the check, with a control: the same scenario delivered once, in order, or without the crash passes.
3. Check source-visible protection before reporting: an idempotency key stored atomically with the effect, a version or sequence check, a fencing token, a transactional outbox, or a broker setting fixed in repository configuration. If it covers every real path, reject.
4. A theoretical message loss with no crash point or redelivery path that reaches it is a hardening note, not a candidate.
5. Return `needs_validation` when the defect depends on deployed broker or provider configuration, instance counts, real network or replication timing, or a third-party retry policy, and give the bounded fake-transport scenario or owner-observed setting that would decide it.
