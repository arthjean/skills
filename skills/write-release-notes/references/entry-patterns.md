# Entry Patterns

Use for category ambiguity or vague entries in [`write-release-notes`](../SKILL.md). Examples are fictional; use only identifiers and measurements supported by the actual release.

## Classifying into the six categories

Keep a Changelog 1.1.0 fixes the set: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security` (<https://keepachangelog.com/en/1.1.0/>). Ambiguity resolves by asking what the reader must do:

| The reader must | Category |
|---|---|
| Adopt something that did not exist | `Added` |
| Adjust to different behavior from the same call | `Changed` |
| Plan a migration before a future release | `Deprecated` |
| Migrate now, because the old path is gone | `Removed` |
| Nothing, the bug they hit is gone | `Fixed` |
| Upgrade urgently, with an affected range to check | `Security` |

A single pull request often produces rows in two categories. Split it: `Added` the new API, `Deprecated` the old one, in the same release.

## The so-what rewrite

Lead with the effect on the reader, then the mechanism. The mechanism alone forces every reader to simulate the consequence themselves, and an agent to guess it.

```
Before: Refactored the token bucket implementation in RateLimiter.
After:  Burst traffic no longer returns 429 for the first second after a
        cold start. RateLimiter now pre-fills its bucket (#4412).
```

```
Before: Improved performance of the query planner.
After:  Aggregation queries over partitioned tables plan 4x faster
        (p95 210ms to 52ms on the 1M-row benchmark) (#4380).
```

```
Before: Fixed a bug in session handling.
After:  Sessions created before a server restart are no longer silently
        dropped; `SessionStore.load()` now rehydrates from the backing
        store instead of returning `null` (#4401).
```

```
Before: Updated the auth middleware to be more flexible.
After:  BREAKING: `authenticate()` now returns `Promise<Session>` instead
        of `Session`. Await the call, or use `authenticateSync()` for the
        previous behavior.

        - const session = authenticate(req)
        + const session = await authenticate(req)
```

## Behavior changes

The class both readers miss, because no signature moves. Assess compatibility even when the signature is unchanged: the old call still compiles and now does something different.

State the trigger condition, the old result, the new result, and the opt-out where one exists.

```
`parseDate()` now interprets bare `YYYY-MM-DD` strings as UTC rather than
local time. Values crossing a day boundary shift by up to 24 hours. Pass
`{ tz: "local" }` to keep the previous behavior (#4455).
```

## Quantify what invites a number

Performance, size, cost, and limits are claims a reader will otherwise verify by hand. When measurements exist, give the metric, delta, and measurement basis. Otherwise describe only the supported effect.

## Evidence links

Link available PRs or commits for what changed, related issues for context, and documentation for usage. Use non-closing issue references unless closure is explicitly requested. A fixed link count is unnecessary.

For longer notes, possible editorial choices include: always date, make every release linkable, group under category headers, open with a paragraph on the themes, show examples and screenshots, credit contributors according to project policy (<https://simonwillison.net/2022/Jan/31/release-notes/>).

## Anti-patterns

Keep a Changelog names the first four; the rest recur in practice.

- **Commit log dump.** `git log` output is a record of work, not of change. Ship the ledger, not its source.
- **Selective documentation.** Omitting inconvenient changes costs the whole document its credibility. Breaking changes omitted from notes are discovered in production.
- **Ignored deprecations.** A removal that was never announced reads as a bug to everyone downstream.
- **Regional dates.** `03/04/2026` is two dates. ISO 8601 is one.
- **"Various bug fixes and improvements."** Zero information at full token cost. Enumerate or omit the section.
- **Implementation-first phrasing.** Internal class and module names in a document read by people who cannot see them.
- **Version-less prose.** "Recently we changed" and "in an upcoming release" leave the reader with no version to pin.
- **Marketing register in a technical changelog.** Superlatives push out the identifiers an agent needs. Keep the launch narrative in the announcement post and link it.
- **Stock phrasing.** Canned transitions, "it's worth noting", contrastive framing that introduces an alternative nobody raised ("not X, but Y"), and closing lines that restate the entries. State the change and stop.

## Generated or authored

The ecosystem splits, and both halves are defensible:

- **Generated from commits**: semantic-release (<https://github.com/semantic-release/semantic-release>), release-please (<https://github.com/googleapis/release-please>), git-cliff (<https://git-cliff.org>), all keyed on Conventional Commits (<https://www.conventionalcommits.org/en/v1.0.0/>).
- **Authored per change**: Changesets (<https://github.com/changesets/changesets>), where each pull request carries a human-written, user-facing summary and its version bump.

Follow whichever the project already uses. When both are open, generate the exhaustive layer from commits and author the human layer by hand: the generated list guarantees the ledger is complete, the authored prose passes the so-what rewrite.
