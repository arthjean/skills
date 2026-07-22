import { resolve } from "node:path";

type EvalCase = {
  id: string;
  agent: string;
  risk: string;
  prompt: string;
  assertions: string[];
};

type EvalDocument = {
  version: number;
  cases: EvalCase[];
};

type JsonRecord = Record<string, unknown>;

function argumentValue(name: string): string | undefined {
  const index = Bun.argv.indexOf(name);
  return index >= 0 ? Bun.argv[index + 1] : undefined;
}

function command(commandArgs: string[], cwd: string) {
  return Bun.spawnSync(commandArgs, { cwd, stdout: "pipe", stderr: "pipe" });
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function containsAgentType(value: unknown, agent: string): boolean {
  if (Array.isArray(value)) return value.some((item) => containsAgentType(item, agent));
  if (!isRecord(value)) return false;
  for (const [key, child] of Object.entries(value)) {
    if ((key === "agent_type" || key === "agentType") && child === agent) return true;
    if (containsAgentType(child, agent)) return true;
  }
  return false;
}

function numberAt(record: JsonRecord, key: string): number {
  return typeof record[key] === "number" ? record[key] : 0;
}

const caseId = argumentValue("--case");
const cwdInput = argumentValue("--cwd");
const shouldRun = Bun.argv.includes("--run");
if (!caseId || !cwdInput) {
  console.error("Usage: bun run.ts --case <id> --cwd <disposable-git-fixture> [--run]");
  process.exit(2);
}

const casesUrl = new URL("./cases.data", import.meta.url);
const document = Bun.TOML.parse(await Bun.file(casesUrl).text()) as EvalDocument;
const testCase = document.cases.find((item) => item.id === caseId);
if (!testCase) {
  console.error(`Unknown case: ${caseId}`);
  process.exit(2);
}

const cwd = resolve(cwdInput);
const evalPrompt = [
  "Run one custom-agent evaluation.",
  `Spawn exactly the custom agent type \"${testCase.agent}\" with no conversation history.`,
  "Pass only the task below, wait for the child, and return its final answer verbatim.",
  "Do not perform the task in the parent and do not spawn any other agent.",
  "",
  testCase.prompt,
].join("\n");

if (!shouldRun) {
  console.log(JSON.stringify({ case: testCase, cwd, command: "codex exec", prompt: evalPrompt }, null, 2));
  process.exit(0);
}

if (!(await Bun.file(resolve(cwd, ".agent-eval-fixture")).exists())) {
  console.error("Refusing to run without a .agent-eval-fixture marker in the target directory.");
  process.exit(2);
}

const rootResult = command(["git", "rev-parse", "--show-toplevel"], cwd);
if (rootResult.exitCode !== 0 || resolve(rootResult.stdout.toString().trim()) !== cwd) {
  console.error("The target must be the root of a disposable Git repository.");
  process.exit(2);
}

const beforeStatus = command(
  ["git", "status", "--porcelain=v1", "--untracked-files=all"],
  cwd,
).stdout.toString();
if (beforeStatus.trim()) {
  console.error("The disposable fixture must be clean before the evaluation.");
  process.exit(2);
}

const startedAt = performance.now();
const result = Bun.spawnSync(
  ["codex", "exec", "--strict-config", "--ephemeral", "--json", "-C", cwd, evalPrompt],
  { cwd, stdout: "pipe", stderr: "pipe" },
);
const elapsedMs = Math.round(performance.now() - startedAt);
const stdout = result.stdout.toString();
const events: JsonRecord[] = [];
for (const line of stdout.split("\n")) {
  if (!line.trim()) continue;
  try {
    const parsed = JSON.parse(line) as unknown;
    if (isRecord(parsed)) events.push(parsed);
  } catch {
    continue;
  }
}

let inputTokens = 0;
let cachedInputTokens = 0;
let outputTokens = 0;
let webSearches = 0;
let context7Calls = 0;
let finalOutput = "";
for (const event of events) {
  if (event.type === "turn.completed" && isRecord(event.usage)) {
    inputTokens += numberAt(event.usage, "input_tokens");
    cachedInputTokens += numberAt(event.usage, "cached_input_tokens");
    outputTokens += numberAt(event.usage, "output_tokens");
  }
  if (event.type !== "item.completed" || !isRecord(event.item)) continue;
  const itemText = JSON.stringify(event.item);
  if (event.item.type === "web_search") webSearches += 1;
  if (event.item.type === "command_execution" && itemText.includes("ctx7")) {
    context7Calls += 1;
  }
  if (event.item.type === "agent_message" && typeof event.item.text === "string") {
    finalOutput = event.item.text;
  }
}

const afterStatus = command(
  ["git", "status", "--porcelain=v1", "--untracked-files=all"],
  cwd,
).stdout.toString();
const summary = {
  case_id: testCase.id,
  requested_agent: testCase.agent,
  requested_agent_observed: events.some((event) => containsAgentType(event, testCase.agent)),
  process_exit_code: result.exitCode,
  elapsed_ms: elapsedMs,
  usage: { input_tokens: inputTokens, cached_input_tokens: cachedInputTokens, output_tokens: outputTokens },
  tool_counts: { web_searches: webSearches, context7_calls: context7Calls },
  fixture_clean: afterStatus.trim().length === 0,
  fixture_changes: afterStatus.trim().split("\n").filter(Boolean),
  assertions_requiring_review: testCase.assertions,
  final_output: finalOutput,
  stderr: result.stderr.toString().trim(),
};
console.log(JSON.stringify(summary, null, 2));

if (result.exitCode !== 0 || afterStatus.trim()) process.exit(1);
