---
name: write-release-notes
description: Write release notes, changelog entries, deprecation notices, or version migration guides.
argument-hint: "[version or range, e.g. v1.4.0 or v1.3.0..HEAD]"
---

# Write Release Notes

A release note is a **contract diff**: what changed for the reader and what they must do about it. Deliver the requested text or file update, with supported claims and actionable migration details. Explicit user instructions take precedence over this guidance.

## Scope and evidence

Use the requested version, range, audience, and destination. Infer missing context from the project's release conventions and available sources; ask only when ambiguity materially changes which release or changes the note covers. Keep an unknown release date or version explicitly unresolved in a draft.

For a full release, account for the changes in the exact base-to-target range, including compatibility, security, runtime requirements, and behavior changes. Group related work into reader-facing entries; keep internal-only changes out of the published note. Use commits, release fragments, contract diffs, and shipped artifacts as applicable. Inspect code or PR context where summaries leave consequential claims unclear. For a single entry or supplied draft, keep investigation scoped to that change.

For a first release, inspect the target tree and relevant history, including the initial commit. Distinguish merged work from what actually ships, including feature flags and platform restrictions. If artifacts contradict the history, report the discrepancy rather than assuming either describes the intended release completely.

## Writing contract

- Follow the project's language, format, attribution, and issue-reference conventions. Lead with observable impact and prioritize breaking changes, security, deprecations, and changed behavior. Scale the introduction and categories to the material.
- Preserve exact versions, public symbols, routes, configuration keys, and advisory IDs. Link available evidence; quantify performance claims only when measurements support them.
- For breaking or action-requiring changes, give the old behavior, new behavior, affected users, and concrete migration steps. Include a code or configuration example when it resolves ambiguity. State when no action or replacement exists.
- Keep changes to the requested documentation. A writing request does not authorize publishing, tagging, changing CI, or executing migrations. Carry out external actions when the user has authorized them; prepare the complete content before any required approval.

## References by need

Read only the reference relevant to the current decision:

- [Release types](references/release-types.md): choosing a structure when the project has no suitable template, or addressing a different audience.
- [Deprecations and breaking changes](references/deprecations.md): documenting a retirement, removal, or compatibility break.
- [Entry patterns](references/entry-patterns.md): resolving category ambiguity or rewriting vague entries.
- [Agent layer](references/agent-layer.md): requested machine consumption, structured metadata, or documentation discovery work.

## Completion

Finish the requested artifact and resolve discrepancies discoverable from the available evidence. Each substantive claim must trace to a source, each relevant change in scope must be represented, and migration instructions must identify the required edits without forcing the reader to reconstruct the diff. Keep remaining unknowns separate from publishable copy, each with where you looked.

Review migration examples against the source. Execute them only when migration validation is part of the authorized task in a suitable environment; report what was inspected versus executed. If missing evidence prevents completion, deliver the supported portion with the precise blocker rather than inventing details or claiming release readiness.

## Sources

Tuned against [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/) (Anthropic, 2026-07-24), [Getting the most out of Opus 5.5](https://claude.dev/blog/getting-the-most-out-of-opus-5-5/#say-what-done-looks-like-then-let-it-run) (Anthropic, 2026-09-22), and [OpenAI model guidance for GPT-6 Astra](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6-astra).
