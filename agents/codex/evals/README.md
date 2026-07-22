# Custom agent evaluations

These cases test routing, contract fidelity, mutation boundaries, and token-cost controls for the three custom agents. The TOML data uses a `.data` extension because Codex recursively treats `.toml` files under `agents/` as role definitions. Evaluation files stay outside automatic instruction discovery, so they add no normal session context.

Validate the suite structure with:

```sh
bun /home/arthur/.codex/agents/evals/validate.ts
```

The validator also compares every currently enabled MCP server from `codex mcp list --json` with each role's explicit deny list. A newly inherited server fails validation until its transport and `enabled = false` entry are added to all research agents.

Preview one behavioral case without spending model tokens:

```sh
bun /home/arthur/.codex/agents/evals/run.ts \
  --case explorer-rejects-implementation \
  --cwd /absolute/path/to/disposable-fixture
```

Execute it by adding `--run`. The target must be a clean Git root containing a `.agent-eval-fixture` marker. The runner uses `codex exec --ephemeral`, requests the exact custom role with no conversation history, records usage and tool counts, and fails if the fixture changes or Codex exits unsuccessfully.

Review the runner summary against the case assertions. Semantic assertions still require judgement; runtime facts such as selected agent, permissions, tool calls, tokens, process status, and filesystem changes come from execution evidence. Keep `gpt-5.6-sol` and record:

- selected agent;
- effective permission profile and exposed tools;
- model and reasoning effort;
- tool calls, opened files or pages, and retrieved bytes;
- final output tokens;
- filesystem diff and external side effects;
- pass or fail for every assertion.

Run mutation cases against a disposable fixture. A passing read-only case leaves the fixture unchanged and shows no unauthorized MCP, network, or messaging action. Run the cold-cache case with an isolated empty Bun cache and verify that only the allowed Bun and Context7 cache/state paths change.

Treat prompt assertions as behavioral expectations, not enforcement. Permission profiles, tool availability, filesystem state, and external side effects must be verified from runtime evidence.
