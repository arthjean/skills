---
name: write-prompt
description: Write or rewrite a prompt for a Claude agent so it is short, clear, and complete. Use when the user asks to write, fix, improve, shorten, or review a prompt, a task brief, or a subagent brief, or pastes a draft prompt to correct.
---

Turn the user's draft or intent into a prompt a current Claude model can run from start to finish. Current models infer method, read the repository, and plan on their own, so a prompt earns its length only through what the model cannot find out by itself. The usual failure is too much text, not too little: extra instructions collide with each other and with the project's standing guidance, and the model follows a needless instruction even where a change of course would serve the user better.

For the reasons behind each rule, the model-specific notes, and the source articles, read [`references/sources.md`](references/sources.md). Read it when the user questions a choice, or when the prompt is a system prompt, a long unattended run, or a brief for an agent harness.

## What a prompt carries

Three things, in this order:

1. **The goal**: what the user wants and, in a clause, why. The reason lets the model generalize to cases the prompt never named.
2. **Done**: the finish line, stated so the model can tell done from not done ("the tests pass", "every endpoint is migrated", "every claim in the report cites a file and line").
3. **Stops**: when to stop and ask, and what is out of bounds ("explore only, change no code", "ask before anything destructive").

Then add only the **unknowns**: what the model cannot learn by looking.

- Preferences and taste: what the user likes, keeps, or rejects.
- Scope edges: which part is wanted and which neighboring part is not.
- Where things are: paths, URLs, the file that shows the wanted behavior.
- References: point at source code or a mockup that already does it, and say what to look for. A reference beats a description.
- The user's starting point when it changes the work: what they already know or tried.

## What to cut

Run the **deletion test** on every line: would the model do the wrong thing without it? If not, delete it.

- Method the model would choose anyway: step lists, how to split the work, report templates, which tool to use.
- Anything the environment already says: `AGENTS.md` or `CLAUDE.md` rules, repository layout, the stack, what an invoked skill defines.
- "Think hard", "think step by step", "be thorough", "be careful": thinking is always on, and effort is a setting, not a sentence.
- Capitals and absolutes (MUST, NEVER, CRITICAL): the model reads "never" as absolute and over-applies it. State the wanted behavior and its reason in a normal voice.
- Role lines and flattery, unless the prompt is a system prompt.
- Examples that pin the answer to one shape. Keep an example only for an exact output format.
- Vague bans ("avoid a generic look"): name the specific things to leave out, or state the positive target.
- The same instruction said twice.

## What to keep when it applies

Each of these changes what the model does, so keep the line when the case fits:

- **Action or advice.** "Change this function" gets an edit; "can you suggest changes" gets suggestions. Use the verb that matches what the user wants.
- **Delegation.** For work across a large codebase, ask to split it across subagents and to check each report's evidence before accepting it. Leave the split itself to the model.
- **Unverified claims.** For research, ask to mark what could not be confirmed and where it looked.
- **Ownership.** Say whether the code is the model's to restructure or the user's to touch as little as possible.
- **Approvals.** Name the exact actions that need a yes. A broad rule makes the model hold routine work.
- **Design.** List the specific styles the user does not want.
- **Long runs.** Ask for the task list in a file, and say which stops are wanted.

## How to work

1. Read the draft for intent. Look up what you can check yourself (paths, file names, what the named skill does) instead of asking the user.
2. Find the ambiguities whose answer would change the result. Resolve them from context where you can; put the rest to the user after the prompt, as short open points. Do not fill a gap with an invented requirement.
3. Write the prompt in the user's language and voice, as plain prose with a short list only for items that are truly separate. Match its length to the task: a few lines for a scoped task, a short page at most for a long unattended run.
4. Apply the deletion test, then the **colleague test**: a capable colleague with no context on this task should be able to follow it without asking a question.

When the user gives a long draft, expect the result to be shorter than what they wrote. When they give one line, expect to add the finish line and the stops, and little else.

## What to return

The prompt in one fenced block, ready to paste. Under it, at most a few lines: what you cut or added and why, and the open points only the user can settle. No walkthrough of the prompt.
