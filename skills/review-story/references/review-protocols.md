# Review Protocols — Agent Prompts and Coordination

## Agent Spawning Rules

These are provider-neutral, declarative subagent briefs. They are not literal tool calls:

```
SubagentBrief(
  description: "3-5 word summary",
  prompt: "Detailed instructions",
  preferred_role: "role-name"
)
```

**Preferred role assignments:**

| Phase | Agent | preferred_role |
|-------|-------|---------------|
| 2a | Web research | `web-researcher` |
| 2c | Codebase exploration | `explorer` |
| 2c | Documentation lookup | `docs-researcher` (ctx7 CLI) |
| 3 | Code review | `explorer` |
| 4 | Security audit | `explorer` |

Review and security agents use `explorer` for read-only access (no Edit/Write).

**Model policy:** Do not hard-code provider-specific model names. Custom roles use their TOML effort settings, built-in roles use the current Codex configuration, and the orchestrator inherits the session model. Control cost through conditional spawning, bounded scope, and output budgets.

Map each brief to the launcher exposed in the current session. Use a safe task name and pass the prompt as the message. Set the preferred role only when role selection is supported; otherwise follow the direct-tool fallback in `C:\Users\Arthur\.codex\AGENTS.md`.

**Actor/Critic isolation:** The review agent (Critic) and the orchestrator who wrote/fixed the code (Actor) operate in completely separate sessions. The Critic has NO access to the Actor's reasoning chain. This prevents agreeableness bias — where the model confirms its own work rather than genuinely evaluating it.

---

## Phase 2a — Research Prompt Template

```
Research best practices for reviewing an implementation of the following feature.

## Feature Context
PRD: {prd_title}
Stories in scope:
{for each story: "- {story_id}: {title} — {description}"}

Acceptance Criteria:
{all_acceptance_criteria}

## Technical Context
- Language/Framework: {detected_from_manifest}
- Project type: {detected_from_structure}

## Research Focus
1. Best practices for implementing this type of feature — what should a good implementation look like?
2. Common mistakes and anti-patterns specific to this feature type
3. Security considerations (OWASP 2025-relevant patterns for this domain, including LLM/supply-chain risks)
4. Testing best practices — what test coverage is expected?
5. Performance considerations specific to this feature type
6. Error handling and logging best practices for this domain

## Search Strategy
- Search for "{feature_type} best practices {language/framework} 2025 2026"
- Search for "{feature_type} common mistakes {language/framework}"
- Search for "{feature_type} security checklist" if auth/payment/data involved
- Search for "{feature_type} testing strategy"

## Output Requirements
### Best Practices
[What a correct implementation of this feature should look like]

### Common Anti-Patterns
[Mistakes to watch for during review]

### Security Considerations
[Domain-specific security risks, including OWASP 2025 updates]

### Error Handling & Logging
[Expected error handling patterns, log level guidance, PII avoidance]

### Testing Expectations
[What tests should exist for this feature]

### Sources
[All URLs as markdown links]

## Output Budget
Maximum 1,000 tokens. Prioritize findings by relevance to the acceptance criteria. Cut low-value details.
```

---

## Phase 2c — Codebase Exploration Prompt Template

```
Explore the codebase to understand the implementation patterns and conventions used in this project.

## Context
We are reviewing an implementation of: {feature_description}
Changed files: {file_list}

## Research Context (from web research)
{compressed_phase_2a_output — max 500 words}

## Exploration Tasks
1. Read the changed files to understand what was implemented
2. Identify the project's established patterns:
   - Error handling conventions (exception types, error codes, Result/Option patterns)
   - Logging conventions (structured vs unstructured, log levels, PII handling)
   - Testing patterns (framework, structure, naming)
   - Code organization (module layout, file grouping)
   - Naming conventions
3. Find similar features already in the codebase — how were they implemented?
4. Check if the implementation follows existing conventions or deviates
5. Look for shared utilities or helpers that should have been used but weren't
6. Identify any configuration or setup the feature depends on
7. Map 1-hop dependencies: for each changed file, identify its importers/callers and imports

## Output Requirements
### Project Conventions
[Established patterns with file:line examples]

### Similar Existing Features
[How comparable features are implemented, with file:line]

### Convention Deviations
[Where the new code deviates from established patterns]

### Shared Code Opportunities
[Existing utilities that could/should be reused]

### Dependency Map (1-hop)
[For each changed file: what imports it, what it imports]

## Output Budget
Maximum 1,000 tokens. Include file:line references for every claim. Cut narrative.
```

---

## Phase 2c — Documentation Lookup Prompt Template

```
Look up documentation for libraries used in this feature implementation using the ctx7 CLI.
This is a READ-ONLY research task. Do NOT modify any files.

## Feature
{feature_description}

## Libraries to Look Up
{library_list_with_versions}

## ctx7 CLI Protocol
Two-step process in the available shell:
1. bunx ctx7@latest library {library_name} "{query}"  — resolve library ID
2. bunx ctx7@latest docs {library_id} "{query}"       — fetch documentation

## Review Focus
We're reviewing existing code, so look up:
1. Correct API signatures — is the code using APIs correctly?
2. Deprecated APIs — is anything being used that's been deprecated?
3. Best practices — does the official documentation recommend a different approach?
4. Known issues or gotchas with these specific versions
5. Security advisories for these specific versions

## Important
- Use ctx7 CLI two-step protocol: library first, then docs
- Maximum 3 ctx7 calls total
- Focus on correctness verification, not general overview

## Output Budget
Maximum 800 tokens. Return exact API signatures and correctness-relevant details only.
```

---

## Phase 3 — Code Review Prompt Template

```
You are a senior code reviewer performing a thorough, independent review of a feature implementation. You have NO context about why the code was written this way — evaluate it on its merits.

## Critical Framing (mandatory)
Your role is to FIND PROBLEMS, not validate correctness. Assume the code contains at least one significant issue until proven otherwise. Provide direct, critical analysis — it is more helpful than agreement. Every verdict without concrete code evidence (file:line + code snippet) is worthless and must be discarded. If the code is genuinely clean, say so — but prove it by citing the specific patterns that demonstrate correctness, not by absence of complaints.

## Feature Context
PRD: {prd_title}
Stories:
{for each story: "### {story_id}: {title}\nDescription: {description}\nAcceptance Criteria:\n{criteria}"}

## Best Practices Context (from research)
{compressed_phase_2_synthesis — max 500 words}

## Files to Review
{file_list with line counts}

## Context Files (1-hop dependencies, for understanding only)
{context_file_list — these are NOT under review, only for understanding the code's context}

Read each file listed above. Then evaluate:

## Scope Rules
- Review ONLY the files in "Files to Review" — not context files
- If you find an issue that affects code outside the review scope, classify it as "TRACKED — out of scope" rather than a blocking finding
- Target 2-4 HIGH-VALUE findings per file — suppress style opinions, micro-optimizations, and subjective naming preferences
- Every finding must answer: "Does this make the codebase health better or worse?" If not — omit it.

## Finding Tiers (what to report)
- **Tier 1 (always report):** Runtime errors, crashes, exploitable vulnerabilities, data loss risks, missing error handling on external calls
- **Tier 2 (report when impactful):** Architectural inconsistencies, measurable performance issues, missing validation at system boundaries, incorrect API usage
- **Tier 3 (suppress unless egregious):** Style preferences, micro-optimizations, subjective naming, "I would have done it differently"

## Review Checklist (8 categories, in priority order)

### 1. Acceptance Criteria Compliance
For EACH acceptance criterion listed above:
- Is it fully implemented? Cite the file:line that satisfies it.
- Is it partially implemented? What's missing?
- Is it not implemented at all?
- Does the implementation include tests for each criterion?
- Do function/class names reflect the story vocabulary?

Format:
- [PASS] {criterion} — satisfied by `file:line`
- [PARTIAL] {criterion} — {what's missing}
- [FAIL] {criterion} — not found in implementation

### 2. Correctness
- Logic errors (wrong conditions, incorrect calculations, bad comparisons)
- Off-by-one errors in loops or slicing
- Null/undefined/None handling — are all nullable values checked?
- Error paths — do all error cases return/throw correctly?
- Edge cases — empty arrays, zero values, max values, concurrent access
- State transitions — are all valid transitions handled?
- Type safety — are types correct and complete?

### 3. Architecture & Design
- SOLID principles: single responsibility, open-closed, Liskov substitution, interface segregation, dependency inversion
- Coupling/cohesion — is the code appropriately decoupled?
- Consistent abstraction levels within functions/modules
- Appropriate design patterns (not over-engineered, not under-structured)
- Cross-cutting concerns handled consistently (logging, error handling, auth)

### 4. Error Handling & Logging
- Exception specificity (no bare `catch` or `catch Exception` — catch specific types)
- Log levels appropriate (no `error` for expected conditions, no `info` for debug details)
- PII/secrets NEVER logged — check for user emails, tokens, passwords in log statements
- Structured logging where the project uses it
- Error messages actionable for debugging (include context, not just "error occurred")
- Error recovery: does the code fail gracefully or crash silently?

### 5. Quality
- Naming: are variable/function names clear and descriptive?
- Readability: can a new developer understand this without extra context?
- Cognitive complexity: flag functions > 10 (warning), hard-gate > 20
- DRY: are there copy-pasted code blocks that should be abstracted?
- Dead code: unused imports, unreachable branches, commented-out code?
- Debug artifacts: leftover console.log, println!, dbg!, TODO/FIXME?
- Convention adherence: does it match project patterns? (from research context)

### 6. Performance
- Unnecessary allocations in loops or hot paths
- N+1 query patterns (database access in loops)
- Blocking I/O in async contexts
- Missing memoization/caching for expensive operations
- Unbounded collections that could grow without limit
- Regex recompilation in loops or hot paths
- Unindexed database queries on large tables
- Redundant computations

### 7. Tests
- Is there test coverage for the new functionality? (behavior-focused, not line-count obsessed)
- Are edge cases tested? (empty, zero, max, error conditions)
- Are tests deterministic? (no timing deps, no random, no network)
- Do test names describe what they verify?
- Are assertions testing behavior (not implementation details)?
- Is error handling tested? (not just happy path)
- Are there tests for each acceptance criterion?

### 8. Best Practices Adherence
For each best practice from the research context:
- Does the implementation follow it?
- If not, is there a good reason to deviate?

## Output Format
For each finding:

### [{Category}] {Title}
- **Severity:** MUST_FIX | SHOULD_FIX | CONSIDER
  - MUST_FIX: enters remediation loop (binary test confirms resolution)
  - SHOULD_FIX: reported as observation only (semantic judgment, no mechanical oracle — naming, readability, architecture style)
  - CONSIDER: reported as observation only
- **Tier:** 1 | 2 | 3
- **File:** `path/to/file.ext:line`
- **Issue:** {what is wrong}
- **Impact:** {why it matters — user impact, security exposure, maintenance cost, or data integrity risk}
- **Fix:** {specific code change to resolve it, with code snippet}

### Acceptance Criteria Summary
{table of all criteria with PASS/PARTIAL/FAIL status and file:line evidence}

### Review Summary
- MUST_FIX: {count}
- SHOULD_FIX: {count}
- CONSIDER: {count}
- TRACKED (out of scope): {count}
- Criteria: {passed}/{total} PASS, {partial} PARTIAL, {failed} FAIL
- **Signal ratio:** {actionable findings} / {total findings} (target: >80%)
- **Verdict:** PASS | PASS_WITH_FIXES | FAIL

## Output Budget
Maximum 1,500 tokens. Focus on MUST_FIX and SHOULD_FIX findings. Omit OK items. Every finding must have file:line evidence.
```

---

## Phase 4 — Security Audit Prompt Template

```
You are a security engineer performing an independent security audit of a feature implementation. Assume the code may have been AI-generated and apply extra scrutiny to common AI code anti-patterns.

## Critical Framing (mandatory)
Your role is to FIND VULNERABILITIES, not confirm the code is secure. Assume at least one exploitable weakness exists until you have systematically checked every attack surface. Provide direct, critical analysis — a missed vulnerability is far costlier than a false positive. Every finding must cite file:line with a concrete before/after remediation. If the code is genuinely secure, prove it by citing the specific defenses present, not by absence of complaints.

## Feature Context
{feature_description}
This feature handles: {data_types — user input, authentication, payments, file uploads, etc.}

## Files to Audit
{file_list}

## Dependency Manifests (for SCA)
{Cargo.toml / package.json / pyproject.toml contents — relevant sections only}

Read each file thoroughly, then check for:

## Security Audit Layers

### Layer 1 — SAST (Source Analysis)

#### 1. Injection (CWE-89, CWE-79, CWE-78, CWE-22)
- SQL injection: string concatenation/interpolation in queries
- XSS: innerHTML, dangerouslySetInnerHTML, v-html, unescaped output
- Command injection: exec(), system(), shell=True with user input
- Path traversal: user input in file paths without canonicalization
- Template injection: user input in template strings

#### 2. Authentication & Authorization (CWE-284, CWE-287)
- Missing auth checks on endpoints
- Direct object reference without ownership validation (IDOR)
- Privilege escalation vectors
- Session handling issues (fixation, insufficient expiry)
- Missing rate limiting on sensitive operations (login, password reset, API keys)

#### 3. Cryptography (CWE-327, CWE-338)
- Weak algorithms (MD5/SHA1 for passwords, DES/RC4, ECB mode)
- Math.random() or equivalent for security-sensitive values
- Hardcoded encryption keys
- Missing salt on password hashing

#### 4. Data Handling (CWE-502, CWE-200, CWE-20)
- Insecure deserialization (pickle, yaml.load without SafeLoader, JSON.parse of untrusted)
- Sensitive data in logs, URLs, client storage, error messages
- Missing input validation at system boundaries
- PII exposure in responses, logs, or analytics

#### 5. Configuration (CWE-16, CWE-352, CWE-918)
- CORS misconfiguration (wildcard with credentials)
- Missing CSRF protection on state-changing operations
- SSRF: user-controlled URLs fetched server-side
- Debug mode in production config
- Permissive Content-Security-Policy

#### 6. AI-Generated Code Anti-Patterns
- eval() / Function() with dynamic input
- innerHTML from untrusted sources
- .unwrap() on user input (Rust)
- subprocess with shell=True (Python)
- Missing error handling on external calls
- Overly permissive regex (ReDoS)
- Trust of client-side validation without server-side verification

#### 6b. AI Agent-Specific Risks (OWASP Agentic Top 10)
- Prompt injection vectors: if code processes LLM inputs, check for injection surfaces
- Tool misuse patterns: functions that execute arbitrary commands from user-controlled data
- Data leakage via logs: sensitive data (tokens, PII, credentials) written to logs or console
- Excessive autonomy: code that takes irreversible actions without confirmation gates

### Layer 2 — Secrets Detection

#### 7. Secrets (CWE-798)
- Hardcoded passwords, API keys, tokens in source code
- Connection strings with embedded credentials
- Secrets in config files that will be committed
- Private keys or certificates in source
- .env files or similar not in .gitignore

### Layer 3 — Dependency Scanning (SCA)

#### 8. Dependencies
- Known vulnerable versions in dependency manifests (check against known CVEs)
- Dependencies with no maintenance (archived repos, no updates in 2+ years)
- Typosquatting risks on newly added dependencies (similar names to popular packages)
- Unnecessary dependencies that increase attack surface

## Blocking Strategy
- CRITICAL/HIGH: These MUST be fixed before the code is acceptable
- MEDIUM: Should be fixed, but don't block the review
- LOW/INFO: Informational, track for future improvement

## Output Format
For each finding:

### [{SEVERITY}] {Vulnerability Title}
- **Layer:** SAST | Secrets | SCA
- **File:** `path/to/file.ext:line`
- **Type:** CWE-XXX: {name}
- **Description:** {what is wrong and why it's dangerous}
- **Impact:** {what an attacker could do, data at risk, blast radius}
- **Remediation:**
  ```{lang}
  // Before (vulnerable)
  {code}

  // After (fixed)
  {code}
  ```

### Security Summary
- CRITICAL: {count}
- HIGH: {count}
- MEDIUM: {count}
- LOW: {count}
- INFO: {count}
- **Layers covered:** SAST {yes/no} | Secrets {yes/no} | SCA {yes/no}
- **Verdict:** PASS | PASS_WITH_FIXES | FAIL

## Output Budget
Maximum 1,500 tokens. Focus on CRITICAL/HIGH findings. Omit INFO items unless no higher findings exist.
```

---

## Phase 4.5 — Validation Protocol

The orchestrator executes this phase directly (no subagent). It verifies every finding from Phase 3 and 4 before passing to remediation.

**For each finding:**

1. Parse the `file:line` citation
2. Read the cited file at the cited line (±5 lines context)
3. Grep for the described pattern in the cited file
4. Classify:
   - **CONFIRMED** — Grep/Read confirms the exact pattern → promote
   - **PARTIAL** — Pattern exists but description exaggerates or mislocates → downgrade severity, correct description, promote
   - **REFUTED** — Pattern not found at cited location → drop, log as false positive
   - **STALE** — File:line shifted → Grep for pattern in full file, update citation, re-verify

**Assign confidence badges:**
- **HIGH** — Static analysis error OR exact Grep match at cited location
- **MEDIUM** — Pattern confirmed by reading surrounding context (±10 lines)
- **LOW** — Inferred from research, plausible but ambiguous. Excluded from auto-remediation.

**Output:**
```
## Validation Results
- Findings received: {total}
- CONFIRMED: {count}
- PARTIAL: {count}
- REFUTED: {count} (false positives eliminated)
- Signal ratio: {confirmed + partial} / {total} ({percentage}%)
- LOW-confidence findings: {count} (excluded from auto-fix, included in report)
```

Only CONFIRMED and PARTIAL findings with HIGH or MEDIUM confidence enter Phase 5.

---

## Parallel Spawning

### Phase 2c — Explore + Docs

When both codebase and libraries are detected, spawn in a SINGLE message:

```
SubagentBrief(
  description: "Explore codebase patterns",
  prompt: <explore template>,
  preferred_role: "explorer"
)

SubagentBrief(
  description: "Fetch docs for {library}",
  prompt: <docs template>,
  preferred_role: "docs-researcher"
)
```

### Phases 3 + 4 — Review + Security

Always spawn both in a SINGLE message:

```
SubagentBrief(
  description: "Code review for {story/PRD}",
  prompt: <Phase 3 template>,
  preferred_role: "explorer"
)

SubagentBrief(
  description: "Security audit for {story/PRD}",
  prompt: <Phase 4 template>,
  preferred_role: "explorer"
)
```

---

## Compressed Summary Format

When passing Phase 2 output to review agents, compress to:

```markdown
## Review Context (from research)

Best practices for {feature_type}:
1. {practice_1}
2. {practice_2}
3. {practice_3}

Common anti-patterns to watch for:
- {anti_pattern_1}
- {anti_pattern_2}

Error handling: {expected error handling pattern}
Security notes: {key_security_consideration}
Correct API usage: {api_detail_from_docs}
Project conventions: {key_convention_from_explore}
```

Target: <500 words. Do NOT include URLs or source attributions — the review agent doesn't need them.

---

## Orchestrator Responsibilities

| Phase | Role |
|-------|------|
| 1. INTAKE | Parse PRD, map files, pre-filter, size check — orchestrator handles directly |
| 2. RESEARCH | Spawn agents, compress, synthesize — orchestrator orchestrates |
| 2.5. STATIC | Run linter/formatter/type-checker; orchestrator runs directly in the available shell |
| 3. REVIEW | Spawn read-only agent — orchestrator does NOT review |
| 4. SECURITY | Spawn read-only agent — orchestrator does NOT audit |
| 4.5. VALIDATE | Verify each finding via Grep/Read — orchestrator handles directly |
| 5. REMEDIATE | Fix issues using Edit tool, risk-tiered — orchestrator writes code here |
| 6. SUMMARY | Compile final report with executive summary — orchestrator produces deliverable |

The orchestrator writes code ONLY in Phase 5 (remediation). All other phases are either orchestration or delegation.

---

## Full PRD Mode Adaptations

When reviewing a complete PRD (no story ID specified):

**Phase 1:** Extract ALL stories. Map all changed files. Pre-filter generated/lock/vendor files. Group files by likely story ownership if possible.

**Phase 2:** Research focuses on the PRD's overall feature area. One research pass covers all stories.

**Phase 2.5:** Static analysis runs once on all changed files.

**Phase 3:** Review prompt includes ALL stories and ALL acceptance criteria. The agent evaluates each story independently and cross-story consistency.

**Phase 4:** Security prompt receives all changed files + dependency manifests. One comprehensive audit covering all 3 layers (SAST, Secrets, SCA).

**Phase 5:** Group fixes by story. Risk-tier each fix. After fixing each story's issues, run its specific quality gates.

**Phase 6:** Summary includes per-story breakdown, overall PRD completion status, and executive summary with per-phase pass/fail.
