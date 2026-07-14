# implement-epic helper protocols

Load this file only when Phase 1 requires an evidence helper or Phase 3 requires an independent DEEP review.

## Routing

| Gap | Preferred role | Scope |
|---|---|---|
| Current external behavior or official security guidance | `web-researcher` | One story and one unresolved decision |
| Exact version-sensitive API or configuration | `docs-researcher` | One library, version, and use case |
| Cross-module integration pattern unresolved by targeted reads | `agent-explorer` | Stop when the integration point is proven |
| DEEP correctness or contract review | Fresh built-in `worker` | Epic diff and one-hop context |
| DEEP sensitive boundary | Fresh built-in `worker` with the security brief | Changed attack surface only |

FAST uses no helper. DEFAULT uses at most one evidence helper and no independent reviewer. DEEP uses at most two evidence helpers for distinct gaps and at most one review axis, except when correctness and security are separate high-risk questions.

Every helper receives only the Proof Ledger, affected story, named gap, and minimum artifact needed. Delegation depth is one. Helpers do not modify files or spawn subagents.

## Evidence helper brief

```text
Objective: Resolve one implementation blocker for {story_id}.

Proof Ledger:
{proof_ledger}

Named gap:
{specific_gap}

Use only the source appropriate to your role. Return at most 400 tokens:
- answer: exact claim
- evidence: file:line, official URL, or documentation ID
- implementation impact: concrete decision
- remaining uncertainty: none or exact blocker

Stop after the gap is resolved. Do not provide an overview, inspect unrelated areas, modify files, or launch another helper.
```

## Independent DEEP review brief

```text
Independently review this epic diff for one named risk axis.

Proof Ledger:
{proof_ledger}

Risk axis:
{public_contract_or_substantial_logic}

Epic diff:
{changed_diff}

Read one-hop context only when the diff is insufficient. Check correctness, acceptance coverage, relevant error paths, regressions, and project conventions.

Return at most 600 tokens. Output only actionable findings:
- severity: MUST_FIX | SHOULD_FIX
- evidence: file:line
- affected story or epic outcome
- concrete fix

Return only `PASS` when no actionable finding exists. Do not modify files or perform a generic repository review.
```

## Independent DEEP security brief

```text
Audit only the named sensitive boundary in this epic diff.

Proof Ledger:
{proof_ledger}

Sensitive boundary:
{security_surface}

Epic diff:
{changed_diff}

Read one-hop context only when required to verify the boundary. Check only relevant authorization, injection, secrets, validation, sensitive-data handling, SSRF, cryptography, destructive operations, or LLM tool risks.

Return at most 600 tokens. Output only actionable findings:
- severity: CRITICAL | HIGH | MEDIUM
- evidence: file:line
- concrete exploit or failure path
- concrete remediation

Return only `PASS` when no actionable finding exists. Do not modify files or expand beyond the named boundary.
```
