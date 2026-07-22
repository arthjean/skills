# Scope Guard (Shared Reference)

## Purpose

Prevents oversized changes from being applied in a single session. Large changes risk incomplete application, difficult rollback, and context exhaustion. When thresholds are exceeded, escalate to a PRD for phased execution.

## Thresholds

| Metric | Direct execution OK | Escalate to PRD |
|--------|-------------------|-----------------|
| Change sets | <= 7 | > 7 |
| Files touched | <= 20 | > 20 |
| Estimated LOC delta | <= 800 | > 800 |
| Cross-service/module | Same module | Multiple services or public API changes |

## Escalation Protocol

> **Non-interactive callers:** Autonomous pipelines that never pause for the user (e.g. `/implement-story`, which runs only on explicit invocation) replace this interactive escalation with **log-and-proceed**: print the scope warning below, then continue. They consume the threshold numbers above, not the AskUserQuestion step. The interactive protocol below applies to callers that can prompt the user.

If ANY threshold is exceeded:

**1. Print scope warning:**

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
SCOPE ALERT - Plan exceeds single-session capacity
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

This change touches **{N} change sets** across **{N} files** (~{N} LOC).
Direct execution risks incomplete application and difficult rollback.

**Recommendation:** Generate a PRD with phased execution via `/write-prd`.
Each phase becomes an independently implementable story.
```

**2. Present options via AskUserQuestion:**

```json
{
  "questions": [{
    "question": "This change is too large for a single session. How would you like to proceed?",
    "header": "Scope",
    "options": [
      { "label": "Generate PRD", "description": "Launch /write-prd to create a phased plan with implementable stories (recommended)" },
      { "label": "Execute anyway", "description": "Proceed with direct execution despite the large scope" },
      { "label": "Reduce scope", "description": "Go back and select only high-priority items to execute now" }
    ]
  }]
}
```

**3. Route based on response:**

- **"Generate PRD"** → invoke `/write-prd` with analysis findings as context. STOP current pipeline.
- **"Execute anyway"** → proceed with the full plan.
- **"Reduce scope"** → ask which items to keep, rebuild plan, re-evaluate scope guard.

## When to Apply

Apply scope guard checks in any pipeline that produces a change plan before execution:
- `/meta-refact` - Step 4d (mandatory, already enforced)
- `/implement-story` - Phase 3b (when plan exceeds thresholds)
