# Agent Boundary Rules (Shared Reference)

## Strict Boundaries

| Agent | CAN do | CANNOT do |
|-------|--------|-----------|
| **agent-explore** | Read code, Grep, Glob, read-only Bash (git log, wc, tree) | Fetch URLs, modify files, run ctx7 CLI |
| **agent-websearch** | Web search (Exa + fallback), WebFetch URLs | Read local code, modify files, run ctx7 CLI |
| **agent-docs** | Run ctx7 CLI (library + docs via Bash), read local manifests | Fetch arbitrary URLs, modify files, search the web |

**Note:** For implement-story Phase 2d documentation lookup, always use `agent-docs` (not `general-purpose`).

## Call Budgets

- **agent-docs**: Max 3 `ctx7` CLI calls per session (library + docs combined)
- **agent-websearch**: Max 8 search tool calls per research task
- **agent-explore**: No fixed limit; cap grep results with `head_limit: 20`

## Output Budget

All agents return at most **1,500 tokens** to the orchestrator (Anthropic Context Engineering: subagents return condensed summaries, not full outputs). Agents must prioritize findings by relevance and cut low-value details.

## Spawning Protocol

All agents spawned via `Agent` tool with `subagent_type`:

```
Agent(
  description: "3-5 word summary",
  prompt: "Detailed instructions with compressed context",
  subagent_type: "agent-explore" | "agent-websearch" | "agent-docs"
)
```
