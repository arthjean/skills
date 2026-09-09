---
name: implement-epic
description: Implement one PRD epic (EP-NNN) through to IN_REVIEW. Use when asked to implement, complete, or finish an epic.
argument-hint: "[prd-path] [EP-NNN] [--profile fast|default|deep]"
---

# implement-epic

Implement: $ARGUMENTS

Finish the epic so that every acceptance criterion is proven from a real execution path, then hand off at `IN_REVIEW`. `review-epic` alone certifies `DONE`. Explicit user instructions override anything below.

## Scope

Resolve the PRD path and epic ID. Without an epic ID, take the first epic with `TODO`, `IN_PROGRESS`, or `BLOCKED` stories. Work the incomplete stories in dependency order; skip `DONE`, `IN_REVIEW`, and `CANCELLED` stories unless reimplementation was requested. Read what you need to locate integration points, nothing more.

Before editing, name the **execution roots** the epic must plug into: route table, command registry, rendered component tree, dependency container, migration list, scheduler, event subscriptions, config readers, public package exports. Missing wiring never appears in a diff, so it must be planned up front.

External evidence (docs, web) is worth fetching only when a current or version-sensitive fact can change the implementation or a test. State the question, use one route, record the fact and its source. Evidence may amend the technical plan; it never rewrites outcome, non-goals, or acceptance criteria without the user. If it contradicts the product contract, report the contradiction.

## Wired means done

- Every new behavior enters through a real execution path in the same pass. An exported symbol nothing calls is unfinished work.
- When new code replaces a path, the old path is removed or redirected. Both wired means runtime behavior did not change.
- Every added config key, environment variable, or flag is actually read, with its default in the manifest the project really loads.
- At least one proof per criterion starts at the entry point. A unit test on the new module alone is not proof.
- Implementation exposing a product-level contradiction stops and reports. A technical correction updates the plan and continues.

## Validation

Validate in proportion to the risk of the changed surface, once, after the last code change:

- Isolated behavior on an established pattern: acceptance tests plus the configured checks for changed files.
- Shared integration, public behavior, or multi-module change: add integration tests and applicable project gates.
- Auth, payments, PII, migrations, untrusted input, crypto, destructive operations, LLM tools, or cross-service behavior: add the relevant full suite and existing security or supply-chain checks.

File count alone never selects the heaviest tier. Local tests, builds, type checks, and linters are authorized without asking; fix failures caused by the epic and rerun the affected checks. A repair after validation invalidates only the dependent evidence. Only the passing run after the final change counts.

## Handoff

Set implemented stories to `IN_REVIEW`, roll up epic status and counters, leave `completed_at` unset, validate the tracker with an existing repository check when one exists. Print `STATUS IN_REVIEW` when every non-cancelled story is implemented, wired, and validated, otherwise `STATUS BLOCKED`.

Receipt: epic and tier, scope corrections, stories implemented, criterion to `file:line` and entry point and proof, changed files, validation results, blockers, and the `review-epic` command to run next.

## Boundaries

Stay inside the epic and preserve unrelated worktree changes. Never weaken tests, fabricate proof, or claim validation from an earlier code state. Never mark `DONE`. Report as `BLOCKED` when the same cause survives two distinct repair approaches, when wiring needs an unapproved architectural decision, or when credentials, external systems, or human-only proof are required. Commit, push, publish, destructive external actions, and scope expansion need explicit authorization.

## Examples

- `/implement-epic tasks/prd-notifications.md EP-002`
- `/implement-epic tasks/prd-billing.md EP-003 --profile deep`
