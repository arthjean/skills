# Three-Tier Constraints Model (Shared Reference)

## Purpose

Classifies agent actions into three risk tiers to balance autonomy with safety. Based on Google's Addy Osmani guidance on constraint models for coding agents.

## The Three Tiers

### ALWAYS (no approval needed)
Actions that are safe, reversible, and local:
- Read files, grep, glob, explore code
- Run tests and linters
- Compress research output before handoff
- Run quality gates (type check, lint, format check)
- Generate plans and reports (read-only analysis)
- Update progress headers and status displays

### ASK FIRST (requires user confirmation)
Actions that affect state, are hard to reverse, or have medium blast radius:
- Delete files or remove significant code blocks
- Push to remote repository
- Modify CI/CD configuration
- Change public API signatures
- Apply fixes classified as HIGH risk (auth, billing, crypto)
- Exceed scope guard thresholds
- Install, remove, or upgrade dependencies (review dependency diff in Phase 5d)
- Execute plans with >7 change sets

### NEVER (hard stops)
Actions that are dangerous, irreversible, or violate trust:
- Commit secrets, credentials, API keys, or .env files
- Force push to main/master
- Skip verification gates
- Run `git reset --hard` or destructive git commands without explicit instruction
- Modify files during a read-only review phase
- Continue past max iteration limits (DDI circuit breaker)
- Silently suppress or downplay security findings
- Stage files with `git add -A` or `git add .`

## How to Apply

Reference this model in any skill that performs actions with varying risk levels. Skills should classify their specific actions into these tiers and enforce the boundaries.

Example in a skill's Constraints section:
```markdown
## Constraints

Applies [Three-Tier Constraints](@~/.claude/skills/_shared/three-tier-constraints.md) model.
Pipeline-specific additions:

- **ALWAYS:** Run static analysis before AI review. Compress context before agent handoff.
- **ASK FIRST:** Apply fixes touching auth/billing logic. Proceed when scope guard thresholds exceeded.
- **NEVER:** Commit without user confirmation. Skip the verification gate after fixes.
```
