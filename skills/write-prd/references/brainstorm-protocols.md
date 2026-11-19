# Brainstorm protocols

Reference for [`write-prd`](../SKILL.md): research briefs for the helpers, challenge prompts for the plan, and the review rubric. Every decision is the skill's own; nothing here is put to the user.

## Contents

- [Research briefs](#research-briefs)
- [Challenge prompts](#challenge-prompts)
- [Review rubric](#review-rubric)

## Research briefs

Each brief states the questions to answer. Drop the questions that do not apply to the feature (an internal tool has no competitive landscape) and add the ones that do.

### Web research (`agent-websearch`)

```text
Research this domain to inform a Product Requirements Document.

Feature: {feature_description}
Domain: {domain}

Questions, most important first:
{the questions this feature raises, for example: how comparable products approach it and what their users praise or criticize; the practices and technical patterns authoritative sources recommend; what users expect at minimum; the security risks specific to this domain and their mitigations; the common failure modes; upcoming changes that should shape the design}

Prefer primary and current sources. If results are thin, return what you have and name the gaps. Answer each question with its sources as markdown links. Mark anything you could not confirm and say where you looked.
```

### Codebase exploration (`agent-explore`)

```text
Map the codebase constraints for a planned feature. Read-only.

Feature: {feature_description}
Research context: {what web research found that bears on the codebase, if any}

Answer: what stack and architecture the feature must fit; how similar features are built and what it can reuse; which patterns it must follow (auth, data, routing, tests) where it touches them; which commands the project's scripts and CI run as quality gates; what guidance the repository gives agents and contributors; which core files the feature must not modify.

Read enough to answer, without touring unrelated implementations. Tie each claim to file:line and separate verified facts from inference.
```

### Documentation lookup (`agent-docs`)

```text
Look up library documentation for a feature being planned, not implemented. Read-only.

Feature: {feature_description}
Libraries: {library_list_with_versions}

For each library: what it offers for this use case, the pattern its official docs recommend, and the limitations or setup that affect the design. Stay on design-relevant facts and name the source of each.
```

## Challenge prompts

Angles to test the plan against before writing. Each concern that survives gets a mitigation, a validation story, or an explicit acceptance in the PRD.

- **Risk**: the top risks research raised, and where teams building similar features struggle.
- **Assumption**: each belief the plan rests on that research does not confirm, or contradicts.
- **Missing consideration**: anything research raised that no decision addressed.
- **Edge cases**, where they apply: empty states, loading states, error states, network degradation, permission changes mid-session, concurrent modifications, boundary values, undo and reversal, interrupted flows, external dependency outages.

## Review rubric

Given to a fresh read-only subagent with the PRD and status file paths. It reports each failed check with the PRD section concerned and what is missing; it does not rewrite the PRD.

| # | Check |
|---|-------|
| 1 | Problem Statement says who has the problem, why it matters, and why now |
| 2 | No subjective word ("fast", "simple", "intuitive") stands where a measurable target is needed |
| 3 | Every number traces to cited evidence or a stated requirement, or is listed as an assumption |
| 4 | Every decision that shapes the design names its evidence |
| 5 | Two engineers reading the PRD independently would build the same thing |
| 6 | Every story has at least one unhappy-path criterion, and each criterion is atomic and verifiable from a real entry point |
| 7 | Each story fits one agent session, and dependencies form no cycle |
| 8 | Each epic has a measurable definition of done |
| 9 | Non-Goals names what a reader would otherwise assume is in scope |
| 10 | Unvalidated beliefs are listed as assumptions, and each high-risk one has a validation story |
| 11 | No section is filler: every optional section present carries content the evidence supports |
| 12 | The status JSON is valid, matches the schema, and lists every epic and story with the PRD's IDs |
