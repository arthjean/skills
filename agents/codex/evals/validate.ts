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

type AgentConfig = {
  model?: string;
  default_permissions?: string;
  mcp_servers?: Record<string, { enabled?: boolean }>;
};

type McpServer = {
  name: string;
  enabled: boolean;
};

const agentNames = [
  "agent-explorer",
  "docs-researcher",
  "web-researcher",
] as const;
const allowedAgents = new Set<string>(agentNames);
const allowedRisks = new Set(["routing", "security", "cost", "contract"]);
const casesUrl = new URL("./cases.data", import.meta.url);
const document = Bun.TOML.parse(await Bun.file(casesUrl).text()) as EvalDocument;
const errors: string[] = [];
const ids = new Set<string>();

if (document.version !== 1) errors.push("version must equal 1");
if (!Array.isArray(document.cases) || document.cases.length < 20) {
  errors.push("at least 20 cases are required");
}
if (Array.isArray(document.cases) && document.cases.length > 50) {
  errors.push("at most 50 cases are allowed");
}

for (const [index, testCase] of (document.cases ?? []).entries()) {
  const label = testCase.id || `case ${index + 1}`;
  if (!testCase.id?.trim()) errors.push(`case ${index + 1}: missing id`);
  if (ids.has(testCase.id)) errors.push(`${label}: duplicate id`);
  ids.add(testCase.id);
  if (!allowedAgents.has(testCase.agent)) errors.push(`${label}: unknown agent`);
  if (!allowedRisks.has(testCase.risk)) errors.push(`${label}: unknown risk`);
  if (!testCase.prompt?.trim()) errors.push(`${label}: missing prompt`);
  if (!Array.isArray(testCase.assertions) || testCase.assertions.length === 0) {
    errors.push(`${label}: at least one assertion is required`);
  }
}

for (const agent of agentNames) {
  const count = (document.cases ?? []).filter((item) => item.agent === agent).length;
  if (count < 6) errors.push(`${agent}: at least six cases are required`);
}

const mcpResult = Bun.spawnSync(["codex", "mcp", "list", "--json"], {
  stdout: "pipe",
  stderr: "pipe",
});
let enabledServers: McpServer[] = [];
if (mcpResult.exitCode !== 0) {
  errors.push(`codex mcp list failed: ${mcpResult.stderr.toString().trim()}`);
} else {
  try {
    const servers = JSON.parse(mcpResult.stdout.toString()) as McpServer[];
    enabledServers = servers.filter((server) => server.enabled);
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    errors.push(`invalid codex mcp list JSON: ${message}`);
  }
}

for (const agent of agentNames) {
  const configUrl = new URL(`../${agent}.toml`, import.meta.url);
  const config = Bun.TOML.parse(await Bun.file(configUrl).text()) as AgentConfig;
  if (config.model !== "gpt-5.6-sol") errors.push(`${agent}: model must be gpt-5.6-sol`);
  if (config.default_permissions !== agent) {
    errors.push(`${agent}: default_permissions must match the agent name`);
  }
  for (const server of enabledServers) {
    if (config.mcp_servers?.[server.name]?.enabled !== false) {
      errors.push(`${agent}: inherited MCP server ${server.name} is not disabled`);
    }
  }
}

if (errors.length > 0) {
  for (const error of errors) console.error(error);
  process.exit(1);
}

console.log(`Validated ${document.cases.length} agent evaluation cases.`);
