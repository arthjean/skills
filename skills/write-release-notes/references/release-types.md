# Release Types

Optional structures for [`write-release-notes`](../SKILL.md). Use the relevant type when existing project conventions do not settle the format. Examples are illustrative; replace identifiers and omit empty sections. These structures do not authorize additional deliverables or publication.

## Library or package

Home: `CHANGELOG.md` at the repository root, mirrored into the GitHub release body for the tag.

````markdown
## [2.4.0] - 2026-08-18

Two changes matter this release: `authenticate()` is now async, and
sessions survive a server restart.

### Breaking changes
- `authenticate()` returns `Promise<Session>`. Await the call, or use
  `authenticateSync()` to keep the previous behavior (#4412).
  ```diff
  - const session = authenticate(req)
  + const session = await authenticate(req)
  ```

### Security
- Fixes session fixation on cookie renewal, GHSA-xxxx-xxxx-xxxx.
  Affects 2.0.0 to 2.3.7, fixed in 2.4.0 (#4460).

### Deprecated
- `LegacyStore` is deprecated and is removed in 3.0.0. Use `SessionStore`,
  which takes the same constructor options (#4433).

### Added
### Changed
### Fixed
````

For a new changelog, the six Keep a Changelog categories and an `Unreleased` section are useful defaults. Link version headings to verified comparison ranges. Preserve an existing format when updating a project.

Reference implementations: Django, Rust, PostgreSQL, Kubernetes (which puts an "Urgent Upgrade Notes" section above everything else).

## API or service

A dated changelog records changes. For deprecations, link an existing lifecycle page where available; see [Deprecations](deprecations.md). Create a separate page only within the requested scope.

Skeleton per entry: date, one line per change, deep link into the reference documentation for each. Anthropic and OpenAI both run exactly this shape (<https://platform.claude.com/docs/en/release-notes/overview>, <https://platform.openai.com/docs/changelog>).

Include where affected by the release:

- The version identifier a caller pins, and whether the change reaches callers pinned to older versions.
- The contract diff artifact alongside the prose: OpenAPI diff, protobuf, or GraphQL schema, per [Agent layer](agent-layer.md).
- New error codes and status codes, spelled exactly, with their trigger condition.
- Rate limit, quota, and pricing changes, which read as breaking to an operator even when the schema is untouched.

## Product or application

Home: a dated changelog post, one entry per release, narrative and visual. Cursor and the GitHub changelog are the current reference shapes (<https://cursor.com/changelog>, <https://github.blog/changelog/>).

What changes versus the library skeleton: entries are grouped by user-facing capability rather than by code category, screenshots or clips can clarify visible changes when available or requested, and the register is the product's own. What stays: the date, the stable per-release link, the explicit call-out of anything that changes existing behavior, and the link to documentation.

Keep an operator section whenever a release moves a self-hosted requirement: minimum runtime version, migration step, downtime window, rollback procedure.

## Model or agent

Separate these concerns where relevant, linking existing documents and creating only the requested artifacts:

- **Changelog entry**: availability and contract. New model IDs, endpoint or parameter changes, pricing, rate limits, deprecations.
- **Migration guide**: what a caller changes to move between two models, published per model pair (<https://platform.claude.com/docs/en/about-claude/models/migration-guide>).
- **System or model card**: capability and safety deltas, with the evals behind them. Link detailed evaluations here; summarize behavior changes that affect upgrades in the changelog. Meta publishes the same shape per family as `MODEL_CARD.md` in the repository (<https://github.com/meta-llama/llama-models>).

Identity rules this type adds:

- **Snapshot versus alias.** Distinguish dated snapshots from floating aliases using the provider's documented guarantees. State which one each change applies to, and recommend pinning a snapshot for production.
- **Channel.** Identify stable, preview, or experimental status and any documented notice policy.
- **Behavior deltas are the payload.** A model that returns different output for the same prompt is the equivalent of a silent behavior change in an API, and needs the same treatment: trigger condition, old behavior, new behavior, and what a caller does about it. Prompt and tool-use changes that require rework belong in the migration guide, linked from the changelog entry.
- **Retirement carries a replacement and a rationale.** Anthropic's deprecation page is the most complete published example, including what it costs users when a model retires (<https://platform.claude.com/docs/en/about-claude/model-deprecations>).

For an agent or CLI product, add the tool surface to the changelog: new or changed tool names and schemas, permission and sandbox defaults, MCP server compatibility, and any change to what the agent may do without asking. Those are the entries an integrator must read before upgrading.
