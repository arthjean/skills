---
name: write-articles
description: Write or review a tech blog article to the bar of the best company engineering blogs.
disable-model-invocation: true
argument-hint: "[topic, source material, or path to a draft]"
---

# Write Articles

An article earns its reader's time by teaching something true they could not easily get elsewhere: a result, a method, a hard-won lesson, or an argument. The reference blogs behind this skill (Delta, Zed, Cursor, Afterquery, Claude) differ in tone and layout, yet every strong article in them honors the same contract. This skill holds that contract fixed and lets the genre decide everything else.

Deliver a **publishable** article as a Markdown file (or the destination's format): no placeholder in the prose, no reference to an asset that does not exist. Beside it, deliver a short report of evidence gaps and figures still to produce. Writing an article never authorizes publishing, pushing, or posting it; that is a separate step the user authorizes explicitly.

When the destination has its own editorial guide (step 1 finds it), that guide wins wherever it is more specific than this skill: its call to action policy, register, front matter, and rules about naming other products.

## The contract

Every article meets every item, whatever its genre. Each item carries its reason, so apply the reason when a case is not covered.

1. **Thesis up front.** The thesis or headline result lands within the first three paragraphs. A launch post's first sentence says what ships and how to get it; an essay may open on a short scene, then turns to its thesis within the first screen. Readers decide on the first screen whether to stay.
2. **Every claim lives in the ledger, and is right.** Numbers, comparisons, quotes, names, dates, chronology, and statements about how code behaves each trace to a source you can point to: benchmark output, code, commit, PR, issue, doc, or the user's own words. Having a source is half the bar; reading it correctly is the other half (step 6 checks both). A claim you cannot source either leaves the article or appears as an honest statement of what is unknown ("we have not measured the UI thread's share yet"), the way Zed and Afterquery write it. Trust is the asset the article builds, and one invented or misread figure spends all of it. Give the exact figure with its unit and conditions ("3.8 s on the Linux tree, cold cache") in place of an adjective.
3. **Show the dead ends.** Include what was tried and abandoned, the hypothesis the data refuted, the constraint that forced the design. The dead end is usually the most instructive paragraph, and it is what makes the final design credible.
4. **State the limits, the gravest first.** Derive limits from the code or data that enforces the behavior, since docs describe intent and code describes fact; when they disagree, follow the code and report the gap to the user. Lead with the limit that matters most to the reader's safety, data, or results (a security boundary's edge cases come before a missing convenience). Honesty about gaps is what separates engineering writing from marketing.
5. **Define at first use.** Give each technical term a plain one-sentence definition where it first appears, then use it consistently. An analogy can carry a hard concept.
6. **Headings that tell the argument.** Read alone, in order, the headings summarize the article: each names a problem, a step, or a finding. Articles over about 1,200 words get a table of contents.
7. **Figures that explain.** A figure exists because prose alone would be slower, and its caption says what to see or what caveat applies. Embed only assets that exist; a figure you cannot produce goes in the figure list, and the prose around its slot reads complete without it.
8. **Every paragraph contributes.** Each paragraph adds a fact, observation, decision, consequence, proof, or instruction the reader has not had yet; one that restates an earlier idea merges or goes. Paragraphs mostly run 30 to 60 words, and a single short sentence on its own line marks a pivot, rarely, so it keeps its force.
9. **An ending that opens.** The last paragraph widens the thesis, names the next problem, or gives one concrete action. It adds something the reader has not read yet.
10. **A named voice.** A named author, "we" for the team's work, "I" for what the author lived. Only crisis communication is signed by the team.
11. **Publication-clean prose.** Plain verbs, concrete nouns, sentences that each carry one claim, and sentences specific to the subject: swap the product's name for a competitor's, and a sentence that stays true needs the behavior or constraint that makes it this product's. Published prose is dash-free: write "cross platform" or rephrase a hyphenated compound, write "3 to 8 ms" for a range, and keep dashes only inside code, inline code, URLs, and `path:line` references. Arthur reads any dash in prose as a machine tell. The lint script in step 5 enforces this along with a list of stock phrases.

## Genres

Pick one genre in the brief, then read only its reference file at the outline step. The genre sets the structure, length, voice, humor, visual density, and ending.

| Genre | Pick it when the article's main promise is | Reference |
|---|---|---|
| Deep dive | how something hard was built, fixed, or made fast | [deep-dive.md](references/genres/deep-dive.md) |
| Research | a measured result: experiment, benchmark, evaluation | [research.md](references/genres/research.md) |
| Essay | an argued position or a new way to see a problem | [essay.md](references/genres/essay.md) |
| Launch | something that ships, and how to use it today | [launch.md](references/genres/launch.md) |
| Guide | a skill the reader applies to their own work | [guide.md](references/genres/guide.md) |
| Incident | what went wrong, who is affected, what was done | [incident.md](references/genres/incident.md) |

When material spans genres, pick the genre of the main promise and borrow a section from another (a launch post can carry a short deep dive).

## Workflow

### 1. Brief

Before any prose, write the brief:

- **Reader**: who they are, what they already know, why they would care.
- **Thesis**: one sentence, either a claim a competent reader could dispute or a result with its number.
- **Takeaway**: what the reader knows or can do after reading.
- **Genre**, **language** (US English unless the user or destination says otherwise), **destination and format** (site, Markdown or MDX conventions, front matter), **author**.
- **House rules**: the destination's own editorial guide and content contract. Search the site's repository for a blog or copywriting guide, content authoring docs, its `AGENTS.md`, and the three most recent posts; note each rule that is more specific than this skill.

Infer each field from the conversation, the repository, and the source material; ask the user only for what you cannot infer, which is usually the thesis or the reader. Done when every field is filled, the house rules are noted (or their absence confirmed), and the thesis passes its one-sentence test.

### 2. Evidence ledger

Gather the material: code, commits, PRs, issues, benchmark outputs, design docs, notes, transcripts. Build the ledger as a file beside the draft (never published): one row per claim the article will make, with its source (path, commit, URL, or command and output) and its status, `verified` or `TODO`. For a performance or quality number you cannot trace, look for the measurement or ask the user for it; a gap stays a gap.

Mine the same material for the dead ends, refuted hypotheses, and forcing constraints of contract item 3. Reverted commits, superseded designs, issue threads, and PR review discussions are where they hide. For the limits of contract item 4, read the code that enforces the behavior and ask what it permits that the docs do not say: which inputs it accepts, which callers it trusts, which cases it refuses.

Done when every number, comparison, quote, and name planned for the article has a row, and the limits list names its gravest item.

### 3. Outline

Read the genre file, then produce:

- **Title**: concrete and specific about what was built, found, or argued. The H1 and the page title are the same string.
- **Dek**: one sentence under the title stating the article's promise.
- **Headings** in order, the opening paragraph's plan, and the ending.
- **Figure list**: each figure with its caption.

Done when the headings alone tell the argument and every caption names what to see.

### 4. Draft

Read [craft.md](references/craft.md), then write the article to the outline and the genre file. Where the ledger has a gap, write around it: drop the claim, or say plainly what is not yet known; the gap itself goes to the delivery report. Length follows the material and the genre file's range, and the article ends when the takeaway is delivered.

### 5. Check

Run the lint script from this skill's base directory:

```bash
python3 <skill-base-dir>/scripts/lint_article.py DRAFT.md --lang en --assets-root <site>/public
```

It reports dashes in prose, leftover `TODO` markers, embedded assets that do not exist, stock phrases, overlong paragraphs, generic headings, and images without alt text, and prints the heading outline with word count. Pass official product names that contain a hyphen with `--allow "GPT-5.5,Wi-Fi"`. Then walk the contract item by item against the draft, and read each paragraph against contract item 8.

Done when the script exits 0 (every remaining warning is a deliberate choice you can name) and each contract item holds.

### 6. Review: cold read and fact check

Spawn two fresh subagents in parallel. If subagents are unavailable, run both passes yourself, one after the other.

**Cold reader.** Give it only the draft, without the brief or the ledger: a reader who shares your context forgives exactly what a stranger will not. Ask it to return:

- the thesis and the takeaway, one sentence each;
- every term used before it is defined;
- every claim that reads as unsupported;
- the paragraph where its attention dropped, and why;
- any heading that does not match its section.

**Fact checker.** Give it the draft and access to the sources (repository, data, docs), with the ledger only as a map of where to look. Ask it to re-derive every load-bearing claim from the source itself (numbers, causes, chronology, how the code behaves, security and safety statements) and return each as confirmed, wrong with the correct reading, or unverifiable. Point it at the errors writers make most:

- statistics read with the wrong meaning (a percentile, a mean over runs, a unit, what a benchmark actually times);
- a quantity that scales stated as a constant;
- dates and the order of events;
- version drift: a feature, setting, or behavior taken from a changelog or an older release that no longer exists at the version the article describes;
- absolutes the code does not guarantee ("only", "always", "both ends must");
- docs that the code contradicts.

Revise until the cold reader's thesis and takeaway match the brief, its lists are empty or answered, and the fact checker reports no wrong claim.

### 7. Deliver

Write the article where the user asked, or to the scratchpad when no location was given. Report the path, word count, the evidence gaps left out of the prose, figures still to produce, discrepancies found between docs and code, and the ledger's path.

## Localization

A translated edition is a full edition: body, headings, captions, figure labels, alt text, title, dek, and page metadata. Code, identifiers, and product names stay as they are. After translating, search the result for sentences and figure text still in the source language; both slip through on major company blogs. Run the lint script with the target `--lang`.

## Reviewing a draft

When the user supplies a draft, review it in place of rewriting it. Run the lint script, check each contract item and the genre file, and return findings ranked by impact, each quoting the passage and giving a concrete fix. Keep the author's voice and structure; fix typos and dashes directly, and rewrite a passage only when asked.
