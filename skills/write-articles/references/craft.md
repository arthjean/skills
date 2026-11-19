# Craft

Techniques drawn from fifteen articles across Delta, Zed, Cursor, Afterquery, and Claude. Each section names the technique, when it pays off, and where to see it done well. Read the model article only when you need to see a pattern in full; the notes here carry what matters.

## Openings

Pick the opening that gets the reader to the thesis fastest for this genre.

- **Result first.** State the headline number in the first paragraph, with its comparison. Zed's Zeta2 post opens on the release and a 30% better acceptance rate; Afterquery's SkyRL post gives 91.2% after 6,400 responses against 91.8% after 51,200 in the introduction; Cursor's token post leads with a 7% cost cut at equal quality.
- **What ships.** First sentence: the capability, who gets it, how to turn it on. Delta's Claude Agent post does this in two sentences.
- **Demystify the title.** When the title names a concept readers half know, correct the misconception at once. Claude's always-on agents guide opens by saying such agents mostly sleep and wake on events.
- **Scene, then turn.** An essay can open on a person, a date, or a confession, provided a short pivot sentence turns to the thesis within the first screen. Zed's Xanadu essay spends five paragraphs on Ted Nelson, then pivots with a three-word sentence about agents arriving.
- **Analogy.** A hard concept can enter through an everyday one. Afterquery introduces self-distillation as a cheat sheet you practice with until you no longer need it.
- **The user-visible behavior.** A deep dive can open on what the reader sees in the product (Delta's thread UI post starts from typing into an agent reply) and then ask how it works.

## Through-line

A deep dive or long essay holds together when one question or phrase recurs at each step. Delta's thread UI post asks at every stage what the least state is that the editor can get away with, which turns a sequence of techniques into one argument and sets up the payoff when the scrollbar forces state back in. Cursor's Git article twice describes its system as correct when degraded and fast when healthy. Choose the through-line from the article's real design principle; state it early, return to it at each turn, and resolve it at the end.

## Building step by step

To explain a system, rebuild it from first principles in numbered steps, where each step works and also exposes the problem the next step solves. Show the core data structure growing one field at a time so the reader sees exactly what each problem cost. Start with the simplest version that runs, then add only what the next problem demands. Derive the teaching version from the real code by removing fields, never by inventing a structure the codebase never had, and say in one sentence what the real one adds. Pseudo-code first when the real code carries incidental detail (Zed's project search shows the whole search as three plain loops of pseudo-code before the real `select_biased!` fix).

## Showing evidence

- **Measure before you fix, and show the measurement.** Zed wrote a CLI benchmark before touching project search and pasted its raw output next to ripgrep's. Raw terminal output, with the command that produced it, is more persuasive than a paraphrased number.
- **Before and after, side by side.** Short videos or paired screenshots of the old and new behavior on the same input.
- **A worked example before the aggregate.** Afterquery's CrossSearch walks one task end to end (the patient file, the public source, the seven grading criteria) before showing any score, so the reader knows what a point of accuracy means.
- **Uncertainty in plain view.** Say when a run used one seed, when a job crashed, when a difference is within noise, and give intervals where you have them. Afterquery's figure captions do this routinely.
- **Every metric defined where first shown**, including direction ("higher is better").
- **Negative results stay in.** An ablation that changed nothing is evidence that the main result is not an artifact.

## Defining terms

One plain sentence at first use, ideally in the same sentence that introduces the term: Zeta2 defines knowledge distillation as a large teacher model generating training data that a smaller student learns to reproduce, then never re-explains it. Cursor defines the DAG, three-phase commit, and the write-ahead log each just before the argument needs it. A term that carries a policy or a legal weight gets a footnote with its exact definition (Afterquery defines "closed model" this way).

## The reader's objection

Name the strongest objection the reader is forming and answer it in the text. Delta's essay confronts the discomfort of teammates seeing your raw prompts and offers a private-first workflow for those who want it. Cursor imagines the reader muttering that UDP is unreliable, agrees, and explains why correctness never depended on it.

## Naming patterns

Give a recurring failure mode a short memorable name and readers will reuse it. Claude's subagent guide calls the system prompt every team appends a sentence to a "Frankenprompt", and ties agent sprawl to Conway's law. Coin a name only for a pattern the article returns to.

## Showing the dead ends

Write the abandoned approach with the same care as the final one: what it was, why it seemed right, the observation that killed it. Delta's thread UI started from Zed's editor architecture and dropped it because its indexes kept drifting. Zed suspected the render loop, built a headless benchmark to test that, and found throughput already matched ripgrep while latency did not. Cursor walks through NFS, GFS, and DRBD before Spokes. Zeta2 trained on commit histories, found them too clean, and switched to real editing traces.

## Humor

Humor comes from the material and the author's own mistakes, in a dry register: Zed admits hubris in refusing ripgrep, Delta's author confesses they cannot spell, Cursor notes that Linus will not come to check how you store packfiles. Self-deprecation reads as confidence; jokes at the reader's or a competitor's expense read as insecurity. Playful devices such as struck-through words suit a playful house voice like Zed's. Incidents, apologies, and policy statements stay plain.

## Captions

A caption completes the sentence "this figure shows that...". Lead with the takeaway, then the conditions: what was measured, on what, how many runs. Afterquery captions name seeds, crashes, and smoothing windows; Delta captions name the action shown in each video. A screenshot with no caption is decoration.

## Interactive figures

When the point is a trade-off that moves with a parameter (replica count against latency, a protocol's phases, a cache's hit rate), an interactive figure lets the reader feel it. Cursor's Git article lets the reader set replicas and latency and step through a three-phase commit and a compare-and-swap race. Specify such a figure in the figure list with its controls, states, and the one insight it must make obvious; build it only when the destination supports it.

## Endings

- **Widen the thesis.** Delta closes the thread UI post by turning a design rule into a claim about software in the age of agents. Zed's Xanadu essay ends on a three-word sentence saying the users are finally here.
- **Name what is still broken.** Zed's project search ends by listing remote search latency, a flooded main thread, and the remaining throughput gap, then invites contributors.
- **One concrete action.** A command to run, a setting to flip, a dataset to download, an address to write to. Claude's model guide ends by telling the reader exactly which task to try and at which effort level.

## Sentences

- One claim per sentence; split a sentence that carries two.
- Active voice with a named actor: "the scheduler starves the full scan", in place of "starvation was observed".
- Numbers with units and conditions; absolute dates ("October 29"), never "last week" in an evergreen post.
- Dates and versions anchor a story, yet a run of them reads as a changelog: keep the ones the argument turns on.
- Short pivot sentences on their own line at real turns in the argument, kept rare; a string of short fragments becomes a mechanical cadence.
- Concrete nouns over category words: "the B+ tree of row heights", over "the data structure".
- Exact vocabulary held constant: once something is a "run", it stays a run.

## Layout to ask the destination for

The reference blogs converge on a reading column of 640 to 700 px (60 to 75 characters per line), body text of 16 to 20 px at 1.5 to 1.75 line height, a dedicated reading typeface, a table of contents that stays visible, a dek under the title, the author and date, and a reading time. When the article goes to a site you control, check that its template provides these; when it does not, say so in the delivery report.
