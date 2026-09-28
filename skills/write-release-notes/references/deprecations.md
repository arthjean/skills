# Deprecations and Breaking Changes

Use for retirements, removals, and compatibility breaks in [`write-release-notes`](../SKILL.md).

## The notice triplet

Every deprecation notice, in any medium, carries three values. A notice missing one of them cannot be acted on:

1. **Notification date**: when the deprecation was announced.
2. **Shutdown date**: when the old path stops working. Where the date is not yet fixed, state a documented floor ("not sooner than YYYY-MM-DD"), which is the form Anthropic uses for model retirement (<https://platform.claude.com/docs/en/about-claude/model-deprecations>).
   If neither a date nor a floor is announced, say so explicitly; do not invent a commitment.
3. **Recommended replacement**: the exact successor, or an explicit statement that none exists and why.

Add the fourth value whenever it exists: the **upgrade path**, executable verbatim.

## Lifecycle states

Four states carry more information than a deprecated flag, and let a reader plan rather than react. Anthropic's model lifecycle is the clearest published instance:

| State | Meaning for the reader |
|---|---|
| Active | Fully supported, recommended for new work |
| Legacy | Still supported, superseded, no longer recommended |
| Deprecated | Retirement announced with a date, migrate now |
| Retired | Requests fail, migration is mandatory |

Google splits the same axis by channel instead: `stable`, `preview`, `latest`, `experimental`, with production pinned to a stable version (<https://ai.google.dev/gemini-api/docs/models>). Channels and lifecycle compose: preview features earn shorter notice periods.

## Notice periods

Use the project's published policy and release history. Verify any external policy before quoting it; notice periods vary by provider, channel, and maturity. If a removal has no prior notice, document that gap and its upgrade impact. Writing notes does not authorize changing the release policy.

## Versioning the rupture

Four strategies, each moving the cost of a breaking change somewhere different. Pick per project and state which one applies:

- **SemVer**: breaking changes gated behind MAJOR, deprecations shipped in MINOR at the earliest (<https://semver.org/spec/v2.0.0.html>).
- **Dated API versions**: the account or request pins a version, and breaking changes only exist in a newer date. Stripe's model, with an upgrade guide per version (<https://stripe.com/blog/api-versioning>, <https://docs.stripe.com/upgrades>).
- **Editions**: breaking changes are opt-in per project and tool-migrated, never imposed on a stable release. Rust's model (<https://doc.rust-lang.org/edition-guide/>).
- **Compatibility promise**: breaking changes are ruled out for the life of the major version. Go 1 (<https://go.dev/doc/go1compat>).

MCP encodes the rupture in the identifier itself: the version string is the date of the last backwards-incompatible change, so comparing two version strings answers the compatibility question without reading prose (<https://modelcontextprotocol.io/specification/2025-11-25/changelog>).

## Where the notice lives

Two complementary surfaces, when present in the project:

- **Changelog**: the flow. Reverse-chronological, one entry per change, the deprecation appears once on the day it is announced.
- **Deprecations page**: the state. A single table of everything currently deprecated with its triplet, plus a section of past retirements. This is the page an agent fetches to answer "is what I am using still supported", and a changelog cannot answer that question, because answering it there requires reading the whole history.

Runtime signals carry the same triplet into the caller's own logs: a deprecation warning naming the replacement, and for HTTP APIs the `Deprecation` and `Sunset` response headers, `Sunset` carrying the shutdown timestamp (RFC 8594, <https://www.rfc-editor.org/rfc/rfc8594.html>).

## Notice checklist

A deprecation notice is complete when it states:

- What is deprecated, spelled exactly as it appears in code or configuration.
- Notification date, shutdown date or floor, recommended replacement.
- The executable upgrade path, or the reason no mechanical migration exists.
- The symptom after shutdown: which error, which status code, which message.
- Whether existing pinned versions keep working, and for how long.
