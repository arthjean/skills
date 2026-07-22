# Research Synthesis Template (Shared Reference)

Standardized output format for multi-agent research pipelines. Used by meta-code and any pipeline combining agent-websearch, agent-explore, and docs agent (ctx7 CLI).

## Output Template

```markdown
## Answer

[Direct, actionable answer - 3-10 sentences. Most important finding first.]

**Confidence:** {high|medium|low} (trajectory: {level}, response: {level}) - {basis}

## Details

### From Web Research
[Key findings with source URLs. Or: "Web research did not yield relevant results."]

### From Codebase Analysis
[Findings with file:line references. Or: "No codebase detected." / "Skipped."]

### From Documentation
[API details and code examples with ctx7 sources. Or: "No library docs needed." / "Skipped."]

### Contested Claims
[Each position with its source. Or: "No contradictions detected across sources."]

## Recommended Approach

[3-7 concrete next steps. Code examples tailored to user's context.]

## Sources
- [Source Title](URL) - annotation
- file:line - what was found
- Library: name vX.Y.Z via ctx7

## Follow-up
[What additional research would materially improve this answer. Or: "No significant gaps identified."]
```

## Source Credibility & Confidence

**Credibility Tiers:**
- **T1 (weight 1.0):** Official docs, primary sources, peer-reviewed papers
- **T2 (weight 0.7):** Engineering blogs from major companies, well-known practitioners
- **T3 (weight 0.4):** Community posts, tutorials, StackOverflow answers
- **T4 (weight 0.2):** Unverified claims, SEO content, undated sources

**Calibration:** Web-only claims (no codebase or docs corroboration) are downgraded one tier.

**Confidence Levels:**
- **High:** Multiple T1/T2 sources agree, codebase evidence confirms, no contradictions
- **Medium:** T2/T3 sources mostly agree, partial codebase evidence, minor contradictions noted
- **Low:** Single source, no codebase evidence, or significant contradictions between sources

## Synthesis Rules

See SKILL.md Steps 5b-5h for the full synthesis rules (citation-first, conflict resolution, triangulation, deduplication, input coverage, compression). These are defined once in the pipeline steps and not repeated here.
