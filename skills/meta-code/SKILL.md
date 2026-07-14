---
name: meta-code
description: "Quota-aware four-phase workflow that analyzes development questions using only the necessary local code, official documentation, current web evidence, or direct synthesis. Use when the user says 'meta-code', '/meta-code', 'research and answer', 'deep research', 'full analysis', or 'comprehensive answer'. Do not use for simple code edits, reproduced bug fixes, or implementing an already-decided specification."
---

# meta-code

Answer the current user request.

## Objective

Produce a grounded development answer with the smallest source and verification topology that can support it.

`ROUTE -> COLLECT -> SYNTHESIZE -> VERIFY/OUTPUT`

For SIMPLE work, do not print phase headers. For STANDARD and DEEP work, print one compact progress line per phase. Never narrate skipped agents or routine checks.

## Profiles

Select the lowest profile that covers the source count, uncertainty, and risk.

| Profile | Use when | Total helper cap | Verification |
|---|---|---|---|
| SIMPLE | Stable single-hop answer, one local question, or one exact source | 0-1 | Orchestrator self-check |
| STANDARD | Two or three answer obligations, mixed local/docs/web evidence, or contextual how-to | 0-2 | Orchestrator self-check |
| DEEP | Architectural or multi-hop analysis, high-stakes decision, unfamiliar domain, volatile comparison, or genuinely contested evidence | 0-3 including verifier and refinement | Orchestrator self-check plus at most one independent verification mode |

Helper caps cover the entire run. A challenge, evaluator, or refinement consumes one slot. Never exceed the cap because a prior helper failed.

## Source routing

| Need | Route |
|---|---|
| Stable fact or conceptual reasoning | Direct synthesis |
| Current project behavior | Targeted `rg` and file reads with `file:line` evidence |
| Bounded codebase flow | Built-in `explorer` only when direct reads are insufficient |
| Broad cross-module architecture or dependency trace | `agent-explorer` |
| Version-sensitive library, SDK, CLI, or cloud API | `docs-researcher` or direct Context7 CLI |
| Current releases, pricing, product state, standards, ecosystem comparison | `web-researcher` |
| Mixed question | Only independent routes tied to distinct answer obligations |

Do not use web research for a fact already established by current official documentation or local code. Do not use documentation lookup for internal project behavior. Do not use a codebase helper for a few targeted reads.

## Phase 1 - ROUTE

1. Extract one to three explicit answer obligations from the user request.
2. Classify the profile and `source_need`: none, codebase, docs, web, or mixed.
3. Resolve only ambiguity that changes the answer. Infer stable project context from manifests and the current directory instead of reformulating every query.
4. Assign each answer obligation to one source route. Remove overlapping routes.
5. Reserve a helper slot for independent verification only when the DEEP question is high-risk, contested, or likely to remain low-confidence.

Do not scan project memory directories. Use relevant memory already supplied by the runtime, but treat stale memory as context rather than current evidence.

**Gate:** Every answer obligation has one necessary source route and the helper budget is fixed.

## Phase 2 - COLLECT

Collect evidence once.

1. Start with direct model knowledge or targeted local inspection.
2. Launch a helper only for a named evidence gap that the parent cannot resolve cheaply. Read [Helper Protocols](references/helper-protocols.md) only when delegation is actually required.
3. Run independent helpers in parallel. Pass at most 200 tokens of task context: the question, assigned obligation, relevant version or path, and expected evidence type.
4. Stop retrieval as soon as every answer obligation has adequate support. Do not seek a third source merely to increase source count.
5. If a helper fails, use sufficient available evidence or record the exact gap. Do not repeat the same request through another helper.

Use primary evidence:
- Current project behavior: local code and command output.
- API contracts: official documentation for the exact version.
- Volatile external claims: current primary sources.
- Community evidence: only for reported behavior not documented by primary sources.

**Gate:** Each answer obligation is supported, explicitly inferential, or recorded as an unresolved gap.

## Phase 3 - SYNTHESIZE

Write one synthesis pass.

1. Lead with the direct answer.
2. Merge duplicate findings and keep only evidence that changes the conclusion, implementation, or trade-off.
3. Match authority to the claim: local code for current implementation, official docs for supported API behavior, and current primary sources for volatile facts.
4. Cite sourced claims inline while writing. Never add a source pointer that did not come from collected evidence.
5. State disagreements only when they change the recommendation. Separate verified facts from inference.
6. Assign `high`, `medium`, or `low` confidence from evidence quality and remaining gaps. Do not calculate a numeric score.

Do not produce separate compression, triangulation, citation-audit, and calibration drafts. These checks belong to the single synthesis pass and Phase 4 self-check.

**Gate:** The draft answers every obligation without unsupported or duplicated claims.

## Phase 4 - VERIFY/OUTPUT

Run one deterministic orchestrator self-check:

- Every answer obligation has a corresponding answer.
- Every sourced factual claim maps to a retrieved URL, documentation ID, command result, or `file:line` pointer.
- Time-sensitive claims use current evidence.
- Code examples match the identified version and project context.
- Recommendations distinguish facts, trade-offs, and inference.
- No source, action, or delegation is fabricated.

For DEEP only, use exactly one independent verification mode when the reserved slot and signal justify it:
- Use a targeted `web-researcher` challenge for one or two contested external claims.
- Use a fresh built-in `worker` evaluator for completeness or reasoning quality when no new evidence is needed.

Never run both. STANDARD and SIMPLE never launch an independent verifier.

If the self-check or verifier finds a blocking gap, perform at most one targeted refinement and never exceed the profile helper cap. Refine only the missing obligation, then update the answer directly. If no slot remains or evidence is unavailable, report the gap instead of opening another research round.

Deliver only the useful answer shape:
- Direct answer or recommendation.
- Essential implementation details or trade-offs.
- Inline citations and a short source list when tools were used.
- Confidence or unresolved gaps only when they affect actionability.

Do not persist research or pipeline metadata to memory unless the user explicitly requests a memory update. Follow the active runtime memory policy when they do.

**Gate:** The answer is complete enough for the selected profile, grounded, concise, and within the fixed helper budget.

## Done When

- [ ] Answer obligations are satisfied or honest gaps are stated.
- [ ] Helper count stays within the selected profile.
- [ ] Sourced claims have real evidence pointers.
- [ ] At most one collection round, one synthesis pass, and one refinement were used.
- [ ] No implicit memory write occurred.

## Error Handling

| Scenario | Action |
|---|---|
| Local evidence is insufficient | Use one scoped codebase helper if the profile allows it |
| Context7 fails or reaches quota | Verify identity when relevant, use official local or web documentation, and state the gap |
| Web evidence conflicts | Present the conflict or use one DEEP challenge slot |
| Helper times out or returns empty | Continue from sufficient evidence; do not retry the same route |
| All required source routes fail | Answer only supported parts and lower confidence |
| Verification finds a non-blocking omission | State it as a gap; do not refine |

## Constraints

- **Always:** Route by actual source need, prefer direct and local evidence, cap helpers across the whole run, cite retrieved evidence, and stop when the answer obligations are supported.
- **Never:** Run a second broad collection, require every source category, launch both a challenger and evaluator, refine more than once, retry failed agents with the same task, write memory implicitly, or produce pipeline telemetry the user did not request.

## Examples

- `/meta-code How does authentication flow through this repository?`
- `/meta-code Compare the current deployment constraints of Cloudflare Workers and Vercel Functions.`
- `/meta-code Verify the correct Next.js API for this installed version and show how it fits this codebase.`

## Reference

- [Helper Protocols](references/helper-protocols.md): load only for delegated evidence, DEEP verification, or targeted refinement.
