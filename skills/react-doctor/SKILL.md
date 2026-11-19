---
model: haiku
name: react-doctor
description: "React codebase health scanner that checks for security, performance, correctness, and architecture issues. Runs react-doctor CLI, interprets results, and provides actionable fix guidance. Use when the user says 'react-doctor', 'check my React code', 'React health check', 'scan React for issues', 'React audit', or after making React changes to catch issues early. Also triggers on: 'fix React performance', 'React best practices check', 'React code quality'. Do NOT trigger for non-React code, general JavaScript questions, or backend-only changes."
argument-hint: "[file-or-folder?]"
allowed-tools: Read, Grep, Glob, Bash(bunx *)
---

# react-doctor — React Codebase Health Scanner

Scan target: $ARGUMENTS

## Overview

react-doctor is a 3-step pipeline that scans React codebases for issues, interprets the results, and provides actionable fix guidance.

1. **Scan** — run `react-doctor` CLI on the target
2. **Interpret** — parse the score and diagnostics, categorize findings
3. **Guide** — provide specific fix guidance for each finding, re-run to verify

## Execution Flow

```
$ARGUMENTS -> [file-or-folder?]
     |
     v
+---------------+
|  Step 1:      |
|  SCAN         |  <- Run react-doctor CLI
+-------+-------+
        |
        v
+-------+-------+
|  Step 2:      |
|  INTERPRET    |  <- Parse score, categorize findings
+-------+-------+
        |
        v
+-------+-------+
|  Step 3:      |
|  GUIDE        |  <- Actionable fixes, re-run to verify
+-------+-------+
```

## Runtime Output Format

Before each step, print a progress header:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[Step N/3] STEP_NAME
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## Step-by-Step Execution

### Step 1 — Scan

Print: `[Step 1/3] SCAN`

**1a. Determine target:**

- If `$ARGUMENTS` contains a path → use that as the scan target
- If `$ARGUMENTS` is empty → scan the current directory (`.`)

**1b. Run react-doctor:**

```bash
bunx -y react-doctor@latest {target} --verbose --diff
```

- `--verbose` provides detailed diagnostics per category
- `--diff` scopes to recently changed files (reduces noise, faster)

If the scan fails (missing dependencies, not a React project), inform the user and suggest checking the project setup.

### Step 2 — Interpret

Print: `[Step 2/3] INTERPRET`

**2a. Parse the output:**

Extract from the react-doctor output:
- **Overall score** (0-100)
- **Category scores** (security, performance, correctness, architecture)
- **Individual findings** with file:line references and severity

**2b. Categorize findings by priority:**

| Priority | Criteria | Action |
|----------|----------|--------|
| CRITICAL | Security vulnerabilities, XSS vectors, unsafe patterns | Fix immediately |
| HIGH | Performance issues, memory leaks, missing error boundaries | Fix before merge |
| MEDIUM | Architecture concerns, missing memoization, prop drilling | Fix recommended |
| LOW | Best practice suggestions, minor optimizations | Fix when convenient |

**2c. Display results:**

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
REACT DOCTOR RESULTS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Score:** {N}/100
**Security:** {N}/100 | **Performance:** {N}/100
**Correctness:** {N}/100 | **Architecture:** {N}/100

**Findings:** {N} total — {N} CRITICAL | {N} HIGH | {N} MEDIUM | {N} LOW
```

### Step 3 — Guide

Print: `[Step 3/3] GUIDE`

**3a. For each finding (CRITICAL and HIGH first):**

- Explain WHY it's a problem (not just what react-doctor flagged)
- Show the specific code that triggered the finding (read the file)
- Provide a concrete fix with before/after code

**3b. After fixes are applied, re-run to verify:**

```bash
bunx -y react-doctor@latest {target} --verbose --diff
```

Compare the new score to the original. Report improvement.

**3c. Summary:**

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
SUMMARY
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

**Score:** {before}/100 → {after}/100 ({+/-delta})
**Fixed:** {N} findings
**Remaining:** {N} findings (with justification)
```

## Hard Rules

1. ALWAYS run `react-doctor` before providing any guidance — never diagnose from memory.
2. Read the flagged files before suggesting fixes — understand the context.
3. Re-run after fixes to verify improvement — never claim a fix without verification.
4. Do NOT modify files during interpretation (Step 2) — only in Step 3.
5. Every finding must include file:line and a specific code fix.
6. If react-doctor is not available or fails, fall back to manual inspection using Read/Grep.

## DO NOT

- Skip the scan and provide generic React advice.
- Suggest fixes without reading the actual flagged code.
- Ignore CRITICAL findings to focus on score improvement.
- Run react-doctor without `--verbose` (insufficient detail for diagnosis).
- Modify unrelated code while fixing flagged issues.

## Done When

- [ ] `react-doctor` scan executed with `--verbose --diff` flags
- [ ] Results parsed and categorized by priority (CRITICAL/HIGH/MEDIUM/LOW)
- [ ] CRITICAL and HIGH findings investigated with file:line context
- [ ] Fixes applied for CRITICAL and HIGH findings with before/after code
- [ ] Re-scan executed to verify improvement
- [ ] Summary displayed with before/after score comparison

## Constraints (Three-Tier)

### ALWAYS
- Run `react-doctor` before providing any guidance — never diagnose from memory
- Read flagged files before suggesting fixes — understand the context
- Re-run after fixes to verify improvement
- Include file:line and specific code fix for every finding

### ASK FIRST
- Nothing — this is a diagnostic and fix workflow

### NEVER
- Skip the scan and provide generic React advice
- Suggest fixes without reading the actual flagged code
- Ignore CRITICAL findings to focus on score improvement
- Modify unrelated code while fixing flagged issues

## References

- [Agent Boundaries](references/agent-boundaries.md) — shared agent delegation rules
- [Three-Tier Constraints](references/three-tier-constraints.md) — ALWAYS/ASK FIRST/NEVER model
