# review-epic helper protocols

Load this file only when Phase 1 requires an evidence helper or Phase 2 requires an independent audit.

## Routing

| Named gap or risk axis | Preferred role | Scope |
|---|---|---|
| Current external behavior or official security guidance | `web-researcher` | One epic criterion and one unresolved claim |
| Exact version-sensitive API or configuration | `docs-researcher` | One library, version, and use case |
| Cross-module integration pattern unresolved by targeted reads | `agent-explorer` | Stop when the integration point is proven |
| DEFAULT correctness review | Fresh read-only reviewer | Epic diff plus one-hop context |
| DEEP correctness or public-contract risk | Fresh read-only reviewer | Named correctness axis only |
| DEEP sensitive boundary | Fresh read-only reviewer with the security brief | Changed attack surface only |

FAST uses no helper. DEFAULT uses at most one evidence helper and one correctness reviewer. DEEP uses at most two evidence helpers for distinct gaps and one primary audit axis. Add the second audit axis only when correctness and security are separate high-risk questions.

Give every helper only the Proof Ledger, named gap or axis, and minimum artifact required. Keep delegation depth at one. Helpers remain read-only and do not spawn subagents.

## Evidence helper brief

```text
Resolve one evidence gap for epic {epic_id}.

Proof Ledger:
{proof_ledger}

Named gap:
{specific_gap}

Use only the source appropriate to your role. Return at most 400 tokens:
- answer: exact claim
- evidence: file:line, official URL, or documentation ID
- review impact: criterion, finding, or profile decision affected
- remaining uncertainty: none or exact blocker

Stop when the named gap is resolved. Keep the task read-only and scoped to the supplied artifact.
```

## Independent correctness brief

```text
Independently audit one implemented epic for correctness.

Proof Ledger:
{proof_ledger}

Risk axis:
{correctness_or_public_contract_axis}

Epic outcome and child criteria:
{epic_contract}

Epic diff:
{changed_diff}

Read one-hop context only when the diff cannot prove the claim. Check cross-story contracts in dependency order, acceptance completeness, relevant unhappy paths, state transitions, regressions, and local conventions.

Return at most 800 tokens and no more than four high-value findings per file. Output only actionable findings:
- severity: MUST_FIX | SHOULD_FIX
- evidence: file:line
- affected story criterion or epic outcome
- concrete failure path
- smallest viable correction

Return only `PASS` when no actionable finding exists. Keep the task read-only and inside the epic boundary.
```

## Independent security brief

```text
Audit only the named sensitive boundary in one implemented epic.

Proof Ledger:
{proof_ledger}

Sensitive boundary:
{security_surface}

Epic outcome and affected criteria:
{epic_contract}

Epic diff:
{changed_diff}

Read one-hop context only when required to verify the boundary. Check applicable authorization, injection, secrets, boundary validation, sensitive-data handling, SSRF, cryptography, destructive operations, dependency exposure, or LLM tool risks. Ignore categories absent from the changed attack surface.

Return at most 800 tokens and no more than four high-value findings per file. Output only actionable findings:
- severity: CRITICAL | HIGH | MEDIUM
- evidence: file:line
- affected story criterion or epic outcome
- concrete exploit or failure path
- smallest viable remediation

Return only `PASS` when no actionable finding exists. Keep the task read-only and inside the named boundary.
```

## Finding validation contract

For each helper finding, the orchestrator must:

1. Read the cited line with enough surrounding context to understand the branch and callers.
2. Confirm the described pattern and failure path against local code.
3. Correct stale locations or exaggerated severity.
4. Drop any claim that local evidence does not support.

Only locally confirmed findings enter remediation or the final receipt.
