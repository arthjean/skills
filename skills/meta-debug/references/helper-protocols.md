# meta-debug helper protocols

Load this file only when Phase 2 delegates a named diagnostic gap.

## Budgets

The selected profile cap overrides the availability of additional roles.

| Role | Diagnostic use | Limit |
|---|---|---|
| `agent-explorer` | Cross-module flow, concurrency, generated boundary, or hidden state | 12-18 operations normally, 24 maximum, 700 output tokens |
| `docs-researcher` | One exact version-sensitive API or configuration question | 3 Context7 calls, 500 output tokens |
| `web-researcher` | Exact external error, known bug, platform issue, or undocumented behavior | 2-4 searches, 700 output tokens |

STANDARD uses at most one helper. DEEP uses at most two helpers for distinct gaps. FAST uses none. Helpers are read-only, use delegation depth one, and never spawn another helper.

When custom role selection is unavailable, use the equivalent direct tool path from the global Codex instructions. Do not claim delegation when it did not occur.

## Diagnostic brief

```text
Objective: Resolve one named diagnostic gap without modifying files.

Failing baseline:
{command_and_core_error}

Relevant context:
{files_versions_and_constraints_under_200_tokens}

Active hypotheses:
{only_hypotheses_this_helper_can_test}

Named gap:
{one_specific_question}

Return only:
- finding: exact causal or eliminative claim
- evidence: file:line, documentation ID, or current URL
- hypothesis impact: supported, eliminated, or unresolved
- fix target: file and behavior, without editing it
- remaining gap: none or one exact blocker

Stop when the named gap is resolved. Do not inventory the repository, provide a framework overview, repeat prior evidence, modify files, or spawn another helper.
```

## Role-specific rules

### agent-explorer

Read the smallest cross-module path that can test the assigned hypotheses. Follow callers, state transitions, configuration, events, and relevant tests. Return direct evidence, not an architecture report.

### docs-researcher

Use Context7 for the exact library and installed version. Resolve the library ID unless it is already supplied, then query the specific API or configuration. Do not inspect unrelated documentation or browse the web.

### web-researcher

Search the exact error with the library, compiler, OS, or tool version. Prefer the official issue tracker, changelog, release notes, and primary reports. Stop when one credible match explains the observed context. Use community sources only when primary sources do not document the behavior.
