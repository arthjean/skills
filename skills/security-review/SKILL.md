---
model: opus
effort: high
name: security-review
context: fork
description: "Security audit of changed code (injection, access control, secrets, configuration) with severity, confidence, and a code fix per finding. Use when asked for a security review, audit, or vulnerability check, or whether a change is safe to ship."
argument-hint: "[file-or-folder?]"
allowed-tools: Read, Grep, Glob, Bash(git diff *), Bash(git log *), Bash(git ls-files *)
---

# security-review

Audit the changed code for exploitable vulnerabilities and report each one with its severity, confidence, and a concrete fix. The audit is read-only: remediation is code in the report, never an edit. Explicit user instructions override anything below.

Think like an attacker: before rating a pattern, ask how it would be exploited from the attack surface. Calibrate in both directions. Code exploitable remotely without authentication is CRITICAL, never softened. When exploitability is uncertain, lower the confidence and flag the finding for human review instead of raising the severity; a missed borderline LOW costs less than a false CRITICAL.

## 1. Scope

Audit the files or folder in `$ARGUMENTS`. Without arguments, take the changed files from git: `git diff --name-only HEAD`, `git diff --name-only --cached`, `git diff --name-only main...HEAD` (or `master`), and untracked new files from `git ls-files --others --exclude-standard`, which have no diff and are audited whole. With no changes and no arguments, or when git fails and no arguments were given, stop and ask which files to audit; never fall back to the whole repository. Skip binary, unreadable, and empty-diff files, and name them in the scope line.

Read every file in scope before auditing. Above about 20 files, read in priority order: auth, session, crypto, database, and user-input handling first, then new files, then the largest changes. Classify language, framework, and risk tier (auth, billing, crypto: HIGH; business logic: MEDIUM; UI and docs: LOW).

## 2. Threat model

Before rating anything, write a 3-5 line threat model from what you read: trust boundaries (where untrusted input enters, where privileged data exits), data flows from source to sink, the attack surface (HTTP handlers, CLI parsers, message consumers, public endpoints, upload handlers), and the risk context of the domain. It sets severity: the same pattern can be CRITICAL in auth code and MEDIUM in an internal CLI.

## 3. Audit

Examine every file in scope through three lenses, using the matching sections of the [security checklist](references/security-checklist.md) for indicators:

- **Code patterns** (sections 1, 5, 8): injection (SQL, XSS, command, path traversal, template), deserialization, input validation, sensitive data exposure, and the anti-patterns generated code tends to introduce.
- **Secrets and configuration** (sections 3, 4, 6, 7): hardcoded credentials and keys, weak crypto, insecure randomness, debug modes, CORS, security headers, CSRF, SSRF, dependency CVEs. Grep for the secret patterns.
- **Logic and authorization** (section 2): broken access control, authentication bypass, IDOR, privilege escalation, missing auth middleware, and ownership checks, evaluated at every trust-boundary crossing in the threat model.

Calibrate against the true positive, false positive, and borderline examples in checklist section 9 before rating. Known-safe patterns (a prepared statement is not SQL injection) are not findings; style and non-security quality are out of scope.

Each finding records severity, confidence, `file:line`, CWE, what is wrong and why it matters, one or two sentences of reasoning for the severity and confidence, and a specific fix as code, with Before and After for CRITICAL and HIGH. "Validate input" is not a fix. When two lenses flag the same line, merge them into one finding at the higher severity.

## 4. Verify

For each finding, re-read the cited line in full context: surrounding guards can make an isolated pattern safe, and a wrong line number voids the finding. Resolve contradictions, such as identical code rated differently in two files. Check reachability from the attack surface: unreachable code is at most INFO. Remove findings that fail; where exploitability stays uncertain, set confidence to LOW and say why.

## 5. Report

Report at most 25 findings, most severe first, and state how many were cut. With no verified finding, say "No security issues found"; never invent one.

````markdown
## Security Audit Report

**Scope:** {N} files analyzed | Language: {lang} | Framework: {framework} | Skipped: {files and reason, or none}
**Threat Model:** {3-5 line summary}
**Risk Summary:** {N} CRITICAL | {N} HIGH | {N} MEDIUM | {N} LOW | {N} INFO
**Must fix before merge:** {CRITICAL and HIGH IDs}
**Flag for human review:** {LOW-confidence IDs, any severity}

### CRITICAL

#### [C-1] {Vulnerability Title}
- **File:** `path/to/file.ext:42`
- **Type:** CWE-XXX: {Vulnerability Name}
- **Severity:** CRITICAL | **Confidence:** {HIGH/MEDIUM/LOW}
- **Description:** {What is wrong and why it is dangerous}
- **Reasoning:** {1-2 sentences: why this severity, why this confidence, exploitability}
- **Remediation:**
  ```{lang}
  // Before (vulnerable)
  {vulnerable_code}

  // After (fixed)
  {fixed_code}
  ```

### HIGH

#### [H-1] {Title}
...

### MEDIUM

#### [M-1] {Title}
...

### LOW / INFO

#### [L-1] {Title}
...

### Summary

- **Total findings:** {N} ({N} verified, {N} pruned in verification, {N} beyond the cap)
- **Recommended fixes:** {MEDIUM IDs}
- **No action required:** {LOW and INFO IDs}
````

## Severity

| Severity | Criteria | Action |
|----------|----------|--------|
| CRITICAL | Exploitable remotely, no auth needed, data breach or RCE risk | Block merge, fix immediately |
| HIGH | Exploitable with some prerequisites, auth bypass, significant data exposure | Block merge, fix before release |
| MEDIUM | Limited exploitability, defense-in-depth violation, information disclosure | Fix recommended |
| LOW | Best practice violation, minor information leak, hardening opportunity | Fix when convenient |
| INFO | Observation, code smell, potential future risk | No action required |

## Confidence

| Confidence | Criteria | Triage impact |
|------------|----------|---------------|
| HIGH | Full dataflow traced source to sink, pattern unambiguous, no mitigating context found, verified | Trust the severity rating |
| MEDIUM | Pattern matches but mitigating context is possible, or dataflow partially traced | Verify manually before acting |
| LOW | Suspicious pattern, but insufficient context to confirm exploitability or whether guards exist elsewhere | Flag for human review regardless of severity |

**Complete when:** every file in scope was read and examined through all three lenses, every reported finding survived verification with an accurate `file:line`, severity, confidence, reasoning, and code fix, and no file was modified.

## Sources

Tuned against [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/) (Anthropic, 2026-07-24), [Getting the most out of Opus 5.5](https://claude.dev/blog/getting-the-most-out-of-opus-5-5/#say-what-done-looks-like-then-let-it-run) (Anthropic, 2026-09-22), and [OpenAI model guidance for GPT-6 Astra](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6-astra).
