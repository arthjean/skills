# meta-code helper protocols

Load this file only when `meta-code` delegates a named evidence gap, DEEP verification, or the single allowed refinement.

## Budgets

The profile helper cap always overrides the availability of more roles.

| Role | Use | Per-helper limit |
|---|---|---|
| `web-researcher` | Current external or contested claim | 2-4 searches normally, 6 maximum, 700 output tokens |
| `docs-researcher` | One exact version-sensitive API question | 3 Context7 calls, 500 output tokens |
| `agent-explorer` | Broad cross-module or dependency question | 12-18 operations normally, 24 maximum, 900 output tokens |
| Built-in `explorer` | Bounded local flow | Smallest useful file set, 600 output tokens |
| Built-in `worker` | DEEP completeness evaluation without new evidence | 500 output tokens |
| Refinement helper | One blocking missing obligation | 400 output tokens and one invocation maximum |

Helpers are read-only, use delegation depth one, and never contact another helper. When the launcher cannot select a preferred custom role, the parent uses the equivalent direct tool path from the global Codex instructions.

## Evidence brief

```text
Objective: Answer one assigned obligation for a development research question.

Question:
{user_question}

Assigned obligation:
{one_answer_obligation}

Known context:
{version_path_or_constraints_under_200_tokens}

Required evidence:
{file_line | official_documentation | current_primary_source}

Return only:
- answer: exact finding
- evidence: real pointer
- impact: how it changes the final answer
- gap: none or one unresolved issue

Stop when the obligation is supported. Do not broaden scope, repeat the question, modify files, or spawn another helper.
```

## DEEP verification selection

Choose one mode only.

### Contested external claim

Use `web-researcher` when one or two external claims require counter-evidence or recency verification.

```text
Challenge only these claims:
{claims_with_existing_sources}

For each claim, return CONFIRMED, WEAKENED, or REFUTED with one current primary source. Use at most one targeted search per claim, or two only for a high-stakes volatile claim. Maximum 500 tokens.
```

### Completeness evaluator

Use a fresh built-in `worker` when the evidence is sufficient but the DEEP synthesis may contain a reasoning or coverage gap.

```text
Evaluate the draft only against these answer obligations:
{answer_obligations}

Draft:
{synthesis_draft}

Evidence pointers:
{source_pointers}

Return PASS or GAP. For GAP, identify the missing obligation, unsupported claim, or reasoning error in at most 500 tokens. Do not collect new evidence or rewrite the answer.
```

## Single refinement

Use only for one blocking gap and only when a helper slot remains.

```text
Resolve this missing obligation only:
{blocking_gap}

Relevant established evidence:
{minimal_evidence}

Return one supported correction with a real evidence pointer in at most 400 tokens. Do not revisit completed obligations or spawn another helper.
```
