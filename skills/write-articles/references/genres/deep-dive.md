# Deep dive

**Promise**: the reader understands how something hard works and why it is built that way, well enough to reuse the reasoning.

## Shape

Pick the arc that matches the story the material actually tells.

- **Build-up.** The behavior the reader sees in the product, then why the obvious approach failed (often the team's own first attempt), then a rebuild from first principles in numbered steps where each step exposes the next problem, then the result with numbers, what remains, and the widened thesis. Model: Delta, [Building a thread UI on hard mode](https://delta.dev/blog/building-a-thread-ui-on-hard-mode).
- **Investigation.** The symptom users reported, the measurement built before any fix, the hypothesis the measurement refuted, the real cause, the fix, the new measurement, then what is still broken. Model: Zed, [Nerd-sniped: Project Search](https://zed.dev/blog/nerd-sniped-project-search).
- **History of a problem.** Why the problem is hard by nature, the possible approaches ordered by complexity, the industry's failed attempts, the standard solution and the flaws time revealed, then the new design and what it unlocks. Model: Cursor, [Git at any scale](https://cursor.com/blog/git-at-any-scale).

## Dials

- **Length**: most land between 1,500 and 4,000 words; only a history arc spanning years earns 7,000. Past 4,000, each section must carry a distinct finding.
- **Voice**: "we" for the team, "I" for the author's own frustration, surprise, or history in the field. An expert who lived the history earns authority by telling it in the first person.
- **Humor**: dry and optional, drawn from the team's own mistakes.
- **Code**: real but minimal; show the lines that change the argument, let the core struct grow field by field, use pseudo-code where real code carries noise.
- **Visuals**: architecture diagrams, before and after video, raw benchmark output in code blocks; an interactive figure when the trade-off moves with a parameter.
- **Through-line**: strongly recommended (see craft.md).
- **Table of contents**: yes.

## What to study in the models

- Delta's thread UI: numbered steps whose headings read as a design log ("Bounding the formatter", "All this for a scrollbar"); the editor struct growing one field per step; inline asides on how the production version goes further than the simplified one; a closing claim that simplicity is hard work.
- Zed's project search: a benchmark harness written before optimizing; the render-loop hypothesis refuted by numbers; the starvation cause explained through one golden file among 100,000; a closing section that lists three remaining problems.
- Cursor's Git article: the three approaches announced up front, then each one's fate; every concept defined just before use; simulators for three-phase commit and the S3 compare-and-swap race; measured throughput (120 pushes per second on S3 Standard, over 300 on Express One Zone).

## Watch for

- **Method before results.** Show how a number was produced before you lean on it.
- **Mechanism before its verification.** The headline mechanism comes first; the tests, fuzzing, and benchmarks that prove it follow the explanation. When a comparison carries known biases, size them, or say which way they push the number.
- **The simplified example is labeled as simplified**, with a sentence on what the real system adds, so experts trust the rest.
- **The product pitch earns its place last.** In a deep dive from a company, the product appears where the design leads to it, after the reader has learned something they keep regardless.
