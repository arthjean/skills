# Frontend and UI State Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds client-side state that changes over time: component trees with effects or lifecycle hooks, client stores (Redux, Zustand, Pinia, signals, context), data-fetching layers and caches, optimistic updates, server rendering with client hydration, persisted client state (localStorage, IndexedDB, cookies read by the client), or long-lived pages that mount and unmount views. The trigger is a source-visible state model or async data flow in the UI, not the mere presence of a frontend framework in the manifest.

Use alongside `BUG-CLASSES.md`. This file covers render and effect lifecycles, request ordering inside a page, and the client state model. Use `CONCURRENCY-AND-ASYNC.md` for event-loop and promise mechanics outside UI components, `DATA-AND-PERSISTENCE.md` for the server store a client cache mirrors, and `NUMERIC-TIME-AND-TEXT.md` for formatting and locale defects in rendered values.

## Core discipline (include in every agent prompt for this domain)

```
- Name the state that goes wrong, where it lives (component, store, cache, URL, storage), and the user-visible consequence: wrong value shown, lost input, action applied twice, stuck spinner, or growing memory.
- Drive the defect through the real component or store seam: render, interact, resolve async work in a chosen order, and assert on what the user sees or what the store holds. A test that calls a private helper does not prove a UI defect.
- Control time and ordering explicitly: fake timers, deferred promises resolved in a chosen order, and an aborted or pending request. Out-of-order and unmount defects need a deterministic_schedule, not a lucky run.
- Framework guarantees are real: batching, keyed reconciliation, automatic request deduplication in the data layer, and abort-on-unmount helpers. Check which ones the code actually uses before claiming a race.
- Use `confirmed` for a red component, store, or hydration test in the local test environment. Use `needs_validation` when the defect depends on real browser behavior the local DOM cannot reproduce (layout, IME composition, focus quirks, back-forward cache, a specific engine).
```

## Render and effect lifecycle bug classes (subagent_type: `general`)

**Stale closure capture**
A callback, timer, interval, or subscription handler captures props or state from the render that created it and keeps using them after newer renders. Typical shapes: an interval reading a counter captured at mount, an event handler registered once with an empty dependency list, a debounced function recreated on every render or never. Prove with a unit_test that renders, changes the captured value, fires the callback (fake timers or a dispatched event), and asserts the stale value is used.

**Missing effect cleanup**
An effect starts work it never stops: a subscription, listener, interval, observer, socket, or animation frame with no teardown, or a teardown that removes a different function reference than the one added. Symptoms are duplicated handlers after remount and memory that grows with navigation. React StrictMode in development mounts, unmounts, and remounts components to surface this class. Prove with leak_measurement: mount and unmount the component N times and count live listeners or subscriptions through a spy on the add and remove calls, expecting zero growth.

**Effect dependency loops and skips**
Dependencies that change identity on every render (inline objects, arrays, functions) re-run an effect forever or refetch on every keystroke; omitted dependencies skip a needed re-run so the view keeps data for the previous id. Prove with a unit_test that counts effect executions or fetch calls across a fixed sequence of renders and asserts the expected count.

**Hydration and render-environment mismatch**
Server and client render different markup because the render reads time, random values, locale, time zone, viewport, or browser-only APIs, or because the client reads persisted state before hydration. The framework may warn, discard the server markup, or attach handlers to the wrong nodes. Prove with an integration_test or harness that renders on the server path, hydrates in the local DOM environment with a different pinned clock or locale, and asserts on the captured hydration warning or the final markup.

## Request ordering and pending work bug classes (subagent_type: `general`)

**Out-of-order response overwrite**
Two requests for the same view are in flight (typing in a search box, switching tabs, paging) and the older response resolves last, overwriting the newer state. Look for fetches with no abort signal, no request identity check, and no data-layer key that discards stale results. Prove with a deterministic_schedule: issue request A then B, resolve B then A through deferred promises, and assert the view shows A's data.

**Optimistic update without rollback**
The UI applies a change before the server confirms it and never reverts on failure, reverts to a snapshot that discards a concurrent change, or rolls back the wrong item in a list. Prove with fault_injection at the client seam: make the mutation fail after the optimistic apply and assert the store or view still shows the unconfirmed value, with a control where the mutation succeeds.

**Work completing after unmount or navigation**
A request, timer, or promise chain resolves after its component unmounted or the route changed, then writes state, navigates, or shows a toast for a page the user left. Prove with a deterministic_schedule: start the work, unmount or navigate, resolve it, and assert the store write, navigation, or notification that should not happen.

**Double submission and repeated activation**
A submit button, keyboard shortcut, or retry path can fire the same action twice before the first completes: the disabled state is set after an await, Enter and click both trigger the handler, or a retry resends without an idempotency key. Prove with a unit_test that activates twice in the same tick, or before resolving the first request, and counts mutation calls.

## Client state model bug classes (subagent_type: `general`)

**Derived state drifting from its source**
A value computed from props or store data is copied into local state once (initial state from props, a memoized selector with incomplete inputs, a cached total) and stops tracking its source. Prove with a property_test or unit_test that applies a sequence of source updates and asserts the derived value equals a fresh recomputation after each step.

**Duplicated source of truth**
The same fact lives in two places (URL and store, server cache and local store, two stores, a form library and component state) with partial synchronization, so they disagree after some event order. Name the two copies and the event that updates only one. Prove with a property_test over action sequences that checks the copies agree, or a unit_test with the specific diverging sequence.

**Persisted client state with an outdated schema**
State restored from localStorage, IndexedDB, or a cookie was written by an older release: renamed fields, a changed enum, a missing version tag, or no migration. The app crashes on load, silently resets user data, or runs with half-valid state. Prove with a unit_test that seeds storage with an old-shape fixture taken from the previous schema in git history and loads the store.

**Controlled input desynchronization**
An input switches between controlled and uncontrolled, formats its value on every keystroke and moves the caret, drops characters during async validation, or resets user edits when props refresh. Prove with a component unit_test that types a sequence and asserts the final value; caret and IME composition effects that the local DOM does not model go to `needs_validation`.

## Universal moves (apply across the above)

- Build a state table per screen: each piece of state, its owner, who writes it, who reads it, what clears it, and which async work can write it after a delay.
- For every async write, ask what happens if it resolves late, twice, after unmount, after a newer request, or after a failure of the request that preceded it.
- Mount, interact, unmount, and remount in loops; compare listener, subscription, timer, and store-subscriber counts before and after.
- Pull the previous persisted shape from git history and load it into the current code.

## Validation rules (apply before reporting ANY finding here)

1. Name the state, the event order or input that corrupts it, and the user-visible consequence. A pattern that looks risky without a reachable event order is a hardening note.
2. Confirm the framework or data layer does not already prevent it: request deduplication, abort on unmount, keyed cache entries, batching, or a library-managed form state.
3. A `confirmed` record needs a red component, store, or hydration test through the real seam, with ordering fixed by fake timers or deferred promises, and a control (requests resolved in order, mutation succeeding, single activation) where practical.
4. For leaks, show growth across repeated mount and unmount cycles, not one retained reference.
5. Use `needs_validation` when the decisive behavior belongs to a real browser engine, device, extension, or deployed caching layer, and name the exact observation an owner can make (a browser profile recording, a specific engine test, a production error log query).
