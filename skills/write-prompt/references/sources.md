# Sources and reasons

Reference for [`write-prompt`](../SKILL.md): why each rule holds, what changes by model and by kind of prompt, and where it comes from. Read on 2026-11-18. Model behavior moves with each release, so when a rule matters and the model is newer than the ones named here, re-read the model's own prompting page before relying on it.

## Contents

- [Why short](#why-short)
- [Goal, done, stops](#goal-done-stops)
- [Unknowns and references](#unknowns-and-references)
- [Lines to cut](#lines-to-cut)
- [Lines that change behavior](#lines-that-change-behavior)
- [System prompts and standing guidance](#system-prompts-and-standing-guidance)
- [Long unattended runs](#long-unattended-runs)
- [Fable 5.2 notes](#fable-52-notes)
- [Source list](#source-list)

## Why short

- Anthropic removed over 80% of the Claude Code system prompt for Claude 5 generation models with no measurable loss on its coding evaluations. The team found it had been overconstraining the model. [S2]
- Instructions from the system prompt, skills, and the user's message collide inside one request ("leave documentation as appropriate" next to "do not add comments"), and the model has to spend thought on the conflict. [S2]
- Too specific, and the model follows the instruction even when a pivot would be better. Too vague, and it falls back on industry defaults that may not fit. The lever between the two is naming the unknowns, not adding detail. [S3]
- A rule that was right for most prompts is wrong for some. Newer models handle the judgment without the rule: "write code that reads like the surrounding code" replaced a paragraph of comment rules. [S2]

## Goal, done, stops

- Give the whole task in one message with the finish line and the point where the model should stop and ask, then let it run. Opus 5.5 and later carry long multi-part work to the end, and a clear finish line tells them when that is. [S1] [S4]
- Anthropic's own demo prompt states the goal and the final output and leaves the approach to the model: it does not say which file to check for which question. [S4]
- Explaining the reason behind an instruction works better than the bare instruction, because the model generalizes from the reason. [S5]
- Sequential steps are worth writing only when the order or the completeness of the steps matters. [S5]
- Define what a successful answer looks like for research tasks. [S5]

## Unknowns and references

- The gap between the prompt (the map) and the real codebase (the territory) is the set of unknowns. Each one the model meets, it settles by guessing. Four kinds: what the prompt says, what the user knows is undecided, what is too obvious to the user to write down, and what the user has not considered. [S3]
- Give the model the user's starting point: where they are in their thinking and how well they know the problem and the code. [S3]
- The best reference is source code: point at the folder and say what to look for, even in another language. It carries far more detail than a screenshot or a description. A test suite, an HTML mockup, or a rubric are references too. [S2] [S3]
- When the user cannot name the unknowns, the prompt can ask for them: a "blind spot pass" for unknown unknowns, an interview one question at a time, or several prototypes to react to. Offer one of these instead of inventing requirements. [S3]

## Lines to cut

- "Think carefully" and "think step by step": the model already thinks before replying and decides how much. Removing the line made replies start sooner with no clear quality drop. How much it thinks is set by the effort level. [S1] [S4] [S6]
- Asking the model to write out its reasoning in the reply can be declined (the `reasoning_extraction` category). Ask for what is needed: "explain why you chose this approach in three sentences". [S1] [S6]
- Forceful wording ("CRITICAL: you MUST") makes current models over-trigger. Use plain wording. [S5]
- An instruction containing "never" is treated as absolute. Write the scoped form: "ask for approval before deleting files", not "never delete files". [S6]
- Examples constrain newer models to a narrow space. Prefer a well-shaped interface or a stated target. Examples remain the reliable tool for an exact output format, tone, or structure, with three to five varied ones. [S2] [S5]
- "Avoid a generic look" swaps one default for another. A list of named patterns to leave out works. [S1] [S4]
- Tell the model what to do rather than what not to do: "flowing prose paragraphs", not "no markdown". [S5]
- Repetition was needed by older models. It can go. [S2]
- Facts the model can see in the file system or the repository do not belong in the prompt. [S2]
- Hard style limits ("20 words or fewer per sentence") make prose choppy, because the model obeys them. [S6]
- Leftover workarounds for older models ("do not be lazy", retry shims, refusal steering) should be removed. [S7]

## Lines that change behavior

- **Action or advice.** Current models follow wording precisely: "can you suggest some changes" may return suggestions only. [S5]
- **Subagents.** Asking to split an audit or migration across subagents, to check each one's evidence, and to finish with one table is a tested pattern. Models delegate on their own, so leave the split to them, and add damping only if they over-delegate. [S1] [S5] [S6]
- **Verification.** Giving the model a way to check its work is the cheapest quality gain: a test, a build, a real run of the changed thing. A separate grader against a rubric improved task success by up to 10 points in Anthropic's internal testing. End-to-end checks are the hardest to fool. [S8] [S9]
- **What could not be confirmed.** "Mark anything you couldn't confirm, and say where you looked." [S1]
- **Ownership.** On Fable 5.2, saying the codebase is the model's own raised consolidation of duplicated code from under 10% of runs to about 75%, with 70 to 80% more output tokens and unrequested changes. Leave it out when the user wants minimal change; a scope rule alone does not cancel it. [S6]
- **Approvals.** Say exactly which actions need a yes. A broad category ("sending messages to people outside this conversation") made the model hold about half of its routine requests. When continuing, say what to do, not "continue". [S6]
- **Review prompts.** "List only problems you'd block the merge for", with file and line, why it is wrong, and how to show it fails. [S1]

## System prompts and standing guidance

A prompt reused across many requests cannot be as specific as a one-off prompt. For these:

- Keep `CLAUDE.md` light: what the repository is for, then mostly gotchas the code does not reveal. Push detail behind pointers (progressive disclosure) into skills and files that load when needed. [S2]
- A role sentence in a system prompt still focuses behavior. [S5]
- Long documents go at the top, the question at the end. Wrap documents and mixed content in XML tags. [S5]
- For reports read by people who did not watch the work, ask for self-contained messages with the key points first and plain words. [S6]
- For standing documents (skills, `AGENTS.md`, `CLAUDE.md`), use the `writing-for-agents` skill instead of this one.

## Long unattended runs

- Put in the standing guidance which stops are wanted: keep going when a step needs no input, stop before anything destructive. [S1]
- Keep the task list in a file, so it survives context summarizing. [S1]
- Keep an implementation-notes file where the model logs deviations from the plan, taking the conservative option and continuing. [S3]
- State and progress belong in files and git: structured data in JSON, notes in free text. [S5]
- For scheduled agents: read from a bookmark rather than a fixed time window, report a failed read as unreadable rather than as "nothing new", re-check items just before acting, and re-read preferences each run. [S10]
- A keep-going rule reduces stops, so the user's own confirmation before risky or irreversible actions stays. [S1] [S6]

## Fable 5.2 notes

- Default effort is high on the API and medium in Claude Code. Most of the gain comes at high; above it gains are uneven. At high and above the model thinks before almost every reply, and an instruction to answer directly does not stop it: lower the effort instead. Effort barely changes reply length; use the prompt for that. [S6]
- At low and medium effort the model may end a turn early by listing what is left or offering to continue. Anthropic publishes a "finishing work" block for system prompts that halved such endings in its tests, at about 30% more output tokens. [S6]
- Cautions pile up in memory, notes, and instruction files, and the model follows them closely. Delete the ones whose reason is gone, and name the routine steps that need no approval. [S6]
- A token budget shown in the prompt changes how much the model spends. Test without it. [S6]
- Model choice is not a prompt matter, but it bounds what a prompt can do: fix verification first, then effort, then the model. [S9]

## Source list

- [S1] [Getting the most out of Opus 5.5 in Claude and Claude Code](https://claude.dev/blog/getting-the-most-out-of-opus-5-5/), claude.dev, 2026-09-22
- [S2] [The new rules of context engineering for Claude 5 generation models](https://claude.dev/blog/the-new-rules-of-context-engineering-for-claude-5-generation-models/), claude.dev, 2026-07-24
- [S3] [A field guide to Claude Fable 5: Finding your unknowns](https://claude.com/blog/a-field-guide-to-claude-fable-finding-your-unknowns), claude.com, 2026-07-06
- [S4] [Using Claude Opus 5.5 as your daily driver for knowledge work](https://claude.com/resources/articles/using-claude-opus-5-5-as-your-daily-driver-for-knowledge-work), claude.com, 2026-11-12
- [S5] [Prompting best practices](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices), Claude Platform docs
- [S6] [Prompting Claude Fable 5.2](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5-2), Claude Platform docs
- [S7] [Building with Claude Sonnet 5.5](https://claude.dev/blog/building-with-claude-sonnet-5-5/), claude.dev, 2026-09-28
- [S8] [How the Claude Code team thinks about verification](https://claude.dev/blog/how-the-claude-code-team-thinks-about-verification/), claude.dev, 2026-11-16
- [S9] [Which Claude model should you build with?](https://claude.dev/blog/which-claude-model-should-you-build-with/), claude.dev, 2026-11-09
- [S10] [Building effective agent automations](https://claude.dev/blog/building-effective-agent-automations/), claude.dev, 2026-10-08

Also read, with nothing specific to prompt writing retained: [Safely scaling multi-agent teams with Claude Fable 5.2](https://claude.dev/blog/safely-scaling-multi-agent-teams-with-fable-5-2/) (a one-sentence role split was enough for 1,000 agents to organize themselves; agents misjudge difficulty and give up early, so encouragement to continue helps) and [Seeing like an agent](https://claude.dev/blog/seeing-like-an-agent/) (tools and instructions that once helped start to constrain as models improve, so revisit them).
