# Research

**Promise**: a measured result the reader can trust, reproduce, and build on.

## Shape

1. **Headline result.** The number or leaderboard first, before the motivation. Afterquery's CrossSearch opens on its interactive leaderboard; the SkyRL post states its key comparison in the introduction.
2. **Motivation through a concrete scenario.** Two or three real situations where the capability matters.
3. **What was built or measured**, in one paragraph: the dataset, the method, the system, the scale.
4. **A worked example.** One instance end to end before any aggregate, so the reader knows what a point of the metric means.
5. **Results**, each figure numbered, each caption carrying the takeaway and the conditions.
6. **Robustness.** Ablations, alternative setups, controls; null results included.
7. **Failure analysis.** Where and why the method or the models fail, with shares.
8. **How it was built.** Data collection, quality gates, review process.
9. **Limits and next steps.**
10. **Release and citation.** Links to data, code, runs; a BibTeX block when the work may be cited.

A lighter variant tells research as a team story, with the problems met as headings ("The reversal problem", "How to know when it's time to ship"). Model: Zed, [How We Developed Zeta2](https://zed.dev/blog/how-we-developed-zeta2).

## Dials

- **Length**: 1,100 to 3,000 words, with detail pushed into collapsible appendices or linked data.
- **Voice**: "we", measured, no humor. Authors listed; affiliations when the work is a collaboration.
- **Apparatus**: numbered sections and figures, references, citation block, methods appendix.
- **Visuals**: charts with defined axes and units, tables for exact values, tabs to switch metrics on one chart.

## What to study in the models

- Afterquery, [CrossSearch](https://www.afterquery.com/blog/crosssearch): leaderboard first; the anticoagulant dosing task as the worked example; an ablation across three search engines where no change was significant at 95%; failure modes as shares of 2,028 failed attempts; a five-stage review process; full release of tasks and runs.
- Afterquery, [On-Policy Self-Distillation in SkyRL](https://www.afterquery.com/blog/on-policy-self-distillation-in-skyrl): a cheat-sheet analogy for the method; captions that state seeds, crashes, and smoothing; a stability finding that started from an accident (a teacher left frozen by mistake).
- Cursor, [Improved token efficiency for longer agent runs](https://cursor.com/blog/improved-token-efficiency): one section per lever, each with its own measured effect (system prompt down 66%, static tool descriptions down 60%, cold cache misses down 20%), plus the history of why each excess existed.
- Zed, Zeta2: knowledge distillation defined in one sentence; one table comparing base models on a named metric; shipping criteria explained (shadow release, gradual rollout).

## Watch for

- **Each metric defined where first shown**, with direction and scale.
- **Sample sizes and seeds stated**; one seed is fine when said.
- **Comparisons at equal conditions**: same data, same budget, same evaluation; say when they differ.
- **The claim matches the evidence**: "matched with one eighth of the samples on this benchmark", in place of a general efficiency claim.
