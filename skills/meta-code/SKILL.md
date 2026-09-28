---
name: meta-code
description: Grounded answer to a development question from local code, official docs, or current web evidence, within a fixed helper budget. Use when the user says meta-code, deep research, or full analysis.
---

# meta-code

Answer the current user request with the smallest set of sources that supports it, then stop. Explicit user instructions override anything below.

## Route by source need

| Need | Route |
|---|---|
| Stable fact or conceptual reasoning | Direct synthesis, no tools |
| Current project behavior | Targeted `rg` and file reads, cited as `file:line` |
| Broad cross-module architecture or dependency trace | `agent-explorer` |
| Version-sensitive library, SDK, CLI, or cloud API | `docs-researcher` or the Context7 CLI |
| Current releases, pricing, product state, standards | `web-researcher` |

Each answer obligation gets one route. Local code settles internal behavior; official docs settle API contracts; web evidence settles only what neither covers. Project memory directories are not evidence: use runtime-supplied memory as context, nothing more.

## Budget

At most three helpers per run, all told, including any verifier or refinement. A failed helper still spends its slot and is never retried through another role. One collection round, one synthesis pass, at most one targeted refinement of a missing obligation. Most questions need zero or one helper; reach three only for a high-stakes, contested, or unfamiliar question. Read [Helper Protocols](references/helper-protocols.md) only when you actually delegate.

An independent verifier (one `web-researcher` challenge on a contested claim, or one fresh evaluator on completeness) is justified only when the answer would otherwise remain low-confidence on a decision that matters.

## Answer

Lead with the direct answer or recommendation. Keep only evidence that changes the conclusion or a trade-off. Cite every sourced claim inline from collected evidence; a pointer that did not come from a tool result is fabricated. Check a helper's pointer yourself before the conclusion rests on it. Separate verified facts from inference, and name disagreements only when they change the recommendation. Give confidence or open gaps only when they affect what the user can do next. Write memory only on explicit request.

**Complete when:** every obligation is answered or its gap is stated with where you looked, every sourced claim maps to a retrieved URL, doc ID, command result, or `file:line`, and the helper count stayed within budget.

## Examples

- `/meta-code How does authentication flow through this repository?`
- `/meta-code Compare the current deployment constraints of Cloudflare Workers and Vercel Functions.`

## Sources

Tuned against [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/) (Anthropic, 2026-07-24), [Getting the most out of Opus 5.5](https://claude.dev/blog/getting-the-most-out-of-opus-5-5/#say-what-done-looks-like-then-let-it-run) (Anthropic, 2026-09-22), and [OpenAI model guidance for GPT-6 Astra](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6-astra).
