const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const {
  LIMITS,
  canonicalCoverageId,
  encodeCanonicalRef,
  isSafeAgentId,
  isSafeRelativePath,
  preflightJsonText,
  readFileWithinLimit,
  validateDocument,
} = require("./validate-coverage-ledger.cjs");

const validatorPath = path.join(__dirname, "validate-coverage-ledger.cjs");
const CLI_TIMEOUT_MS = 5000;
const HOSTILE_CLI_TIMEOUT_MS = 15000;

function unit(overrides = {}) {
  const canonicalRefs = overrides.canonical_refs || {
    entry: "src/api/orders.ts#POST /orders/:id/cancel",
    invariant: "src/orders/state.ts#OrderStatus transitions",
    subsystem: "packages/api",
    bug_class: "BUG-CLASSES.md#State consistency",
  };
  const value = {
    coverage_id: canonicalCoverageId(canonicalRefs),
    canonical_refs: canonicalRefs,
    entry: "Cancel-order route",
    invariant: "Order status transitions only move forward",
    subsystem: "API",
    bug_class: "State consistency",
    starting_paths: ["src/api/orders.ts", "src/orders/state.ts"],
    core_bug_class_block: "BUG-CLASSES.md#State consistency",
    selected_companion_blocks: [],
    excluded_blocks: [{ block: "DATA-AND-PERSISTENCE.md#Cache invalidation", reason: "The route reads no cache." }],
    prior_status: "new",
    attempts: [],
    wave: 1,
    status: "planned",
    agent_id: null,
    reviewed_paths: [],
    local_checks: [],
    result_fingerprints: [],
    unresolved: [],
  };
  return Object.assign(value, overrides, { canonical_refs: canonicalRefs });
}

function errorsFor(value) {
  return validateDocument(value);
}

function runCli(contents, options = {}) {
  const { nodeArgs = [], timeout = CLI_TIMEOUT_MS } = options;
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "validate-coverage-ledger-"));
  const ledgerPath = path.join(directory, "coverage-ledger.json");
  try {
    fs.writeFileSync(ledgerPath, contents);
    return spawnSync(process.execPath, [...nodeArgs, validatorPath, ledgerPath], {
      encoding: "utf8",
      timeout,
    });
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function cliOutput(result) {
  return `${result.stdout}${result.stderr}`;
}

function withTempDirectory(prefix, callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  try {
    return callback(directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function trySymlink(targetPath, symlinkPath, t) {
  try {
    fs.symlinkSync(targetPath, symlinkPath, "file");
    return true;
  } catch (error) {
    if (error && (error.code === "EPERM" || error.code === "EACCES")) {
      t.skip(`symlink creation is not permitted here (${error.code})`);
      return false;
    }
    throw error;
  }
}

const TERMINAL_CONTROL_PAYLOAD = "\u001b\u0007\u0085\u202e";
const TERMINAL_CONTROL_BYTES = [
  Buffer.from([0x1b]),
  Buffer.from([0x07]),
  Buffer.from("\u0085"),
  Buffer.from("\u202e"),
];

function assertNoInjectedControlBytes(output) {
  const bytes = Buffer.isBuffer(output) ? output : Buffer.from(output, "utf8");
  for (const marker of TERMINAL_CONTROL_BYTES) {
    assert.equal(bytes.indexOf(marker), -1, `found raw control bytes ${marker.toString("hex")}`);
  }
}

function sourceCheck(agentId = "hunter-1", overrides = {}) {
  return {
    agent_id: agentId,
    reviewed_paths: ["src/api/orders.ts"],
    invariant: "Cancel rejects orders that already shipped.",
    method: "source",
    result: "The status guard runs before the update.",
    artifact: null,
    ...overrides,
  };
}

function localCheck(agentId = "hunter-1", overrides = {}) {
  return sourceCheck(agentId, {
    method: "local",
    result: "The bounded test canceled an order that had already shipped.",
    artifact: `agents/${agentId}/artifacts/result.txt`,
    ...overrides,
  });
}

function archivedAttempt(overrides = {}) {
  const value = {
    wave: 1,
    status: "blocked",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
    result_fingerprints: [],
    unresolved: ["The production order volume is unavailable."],
    reassignment_reason: "The critic found an unchecked retry path.",
  };
  return Object.assign(value, overrides);
}

test("accepts an empty ledger and complete units", () => {
  assert.deepEqual(errorsFor([]), []);
  assert.deepEqual(errorsFor([unit()]), []);

  const missingAttempts = unit();
  delete missingAttempts.attempts;
  assert(errorsFor([missingAttempts]).some((error) => error.includes('missing required field "attempts"')));

  const covered = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
  });
  assert.deepEqual(errorsFor([covered]), []);
});

test("rejects security-audit field names", () => {
  const legacy = unit();
  legacy.surface = legacy.entry;
  legacy.boundary = legacy.invariant;
  legacy.attack_class = legacy.bug_class;
  legacy.ordinary_attack_class_block = legacy.core_bug_class_block;
  delete legacy.entry;
  delete legacy.invariant;
  delete legacy.bug_class;
  delete legacy.core_bug_class_block;
  const errors = errorsFor([legacy]);
  for (const field of ["entry", "invariant", "bug_class", "core_bug_class_block"]) {
    assert(errors.some((error) => error.includes(`missing required field "${field}"`)), field);
  }

  const legacyRefs = {
    surface: "src/api/orders.ts#POST /orders/:id/cancel",
    boundary: "src/orders/state.ts#OrderStatus transitions",
    subsystem: "packages/api",
    attack_class: "BUG-CLASSES.md#State consistency",
  };
  assert.throws(() => canonicalCoverageId(legacyRefs), /missing or unexpected fields/);
  const legacyRefUnit = unit();
  legacyRefUnit.canonical_refs = legacyRefs;
  assert(errorsFor([legacyRefUnit]).some((error) => error.includes("canonical_refs: contains unexpected fields")));
});

test("accepts a complete ledger through the CLI", () => {
  const result = runCli(JSON.stringify([unit()]));
  assert.equal(result.status, 0, cliOutput(result));
  assert.match(result.stdout, /PASS: 1 coverage units valid/);
});

test("text preflight ignores structural characters and escapes inside strings", () => {
  const value = unit({
    entry: "Route \\ slash [list] {object}, colon: quoted \"value\"",
    excluded_blocks: [{
      block: "COMPANION.md#Literal [brackets] {braces}",
      reason: "The text contains a backslash \\ before an escaped \"quote\".",
    }],
  });
  const contents = JSON.stringify([value]);
  assert.doesNotThrow(() => preflightJsonText(contents));
  assert.deepEqual(JSON.parse(contents), [value]);
  const result = runCli(contents);
  assert.equal(result.status, 0, cliOutput(result));
});

test("text preflight enforces structural cardinality limits", () => {
  const depthLimit = LIMITS.nestingDepth;
  assert.doesNotThrow(() => preflightJsonText(`${"[".repeat(depthLimit)}0${"]".repeat(depthLimit)}`));
  assert.throws(
    () => preflightJsonText(`${"[".repeat(depthLimit + 1)}0${"]".repeat(depthLimit + 1)}`),
    /exceeds nesting depth limit 64/,
  );

  const tooManyUnits = `[${"null,".repeat(LIMITS.units)}null]`;
  assert.throws(() => preflightJsonText(tooManyUnits), /exceeds 10000 top-level unit limit/);

  const tooManyItems = `[[${"null,".repeat(LIMITS.collectionItems)}null]]`;
  assert.throws(() => preflightJsonText(tooManyItems), /exceeds 1000 item array limit/);

  const objectFields = Array.from(
    { length: LIMITS.objectFields + 1 },
    (_, index) => `"field${index}":null`,
  ).join(",");
  assert.throws(() => preflightJsonText(`[{${objectFields}}]`), /exceeds 1000 field object limit/);

  const fullArray = `[${"null,".repeat(LIMITS.collectionItems - 1)}null]`;
  const arraysNeeded = Math.floor(LIMITS.preflightValues / (LIMITS.collectionItems + 1)) + 1;
  const tooManyValues = `[${Array.from({ length: arraysNeeded }, () => fullArray).join(",")}]`;
  assert.throws(() => preflightJsonText(tooManyValues), /exceeds 500000 total value limit/);
});

test("text preflight rejects malformed structural truncation cleanly", () => {
  assert.throws(() => preflightJsonText("["), /truncated JSON structure/);
  assert.throws(() => preflightJsonText("[\"unterminated"), /unterminated JSON string/);
  assert.throws(() => preflightJsonText("[{\"field\":1]"), /mismatched JSON containers/);
});

test("derives collision-free canonical IDs from exact UTF-8 references", () => {
  assert.equal(encodeCanonicalRef("route:POST /orders"), "route%3APOST%20%2Forders");
  assert.notEqual(encodeCanonicalRef("route name"), encodeCanonicalRef("route-name"));
  assert.equal(
    canonicalCoverageId({ entry: "a", invariant: "b", subsystem: "c", bug_class: "d", lifecycle: "retry" }),
    "a::b::c::d::retry",
  );
  assert.equal(
    canonicalCoverageId({ entry: "a", invariant: "b", subsystem: "c", bug_class: "d" }),
    "a::b::c::d",
  );
  assert.throws(() => encodeCanonicalRef("e\u0301"), /invalid canonical reference/);
  assert.throws(() => encodeCanonicalRef("bad\u0000ref"), /invalid canonical reference/);
  assert.throws(() => encodeCanonicalRef("hidden\u200bref"), /invalid canonical reference/);
});

test("rejects noncanonical, duplicate, and colliding IDs", () => {
  const wrong = unit({ coverage_id: "display-label-slug" });
  assert(errorsFor([wrong]).some((error) => error.includes("expected canonical ID")));

  const duplicate = unit();
  assert(errorsFor([duplicate, unit()]).some((error) => error.includes("duplicate coverage ID")));

  const collision = unit();
  const differentMeaning = unit({ entry: "Refund-order route" });
  assert(errorsFor([collision, differentMeaning]).some((error) => error.includes("canonical identity collision")));
});

test("requires canonical references to be own properties", () => {
  const inherited = Object.create(unit().canonical_refs);
  const value = unit();
  value.canonical_refs = inherited;
  assert(errorsFor([value]).some((error) => error.includes("missing required field")));
});

test("rejects aliases for one semantic tuple", () => {
  const first = unit();
  const refs = { ...first.canonical_refs, entry: "src/alias.ts#cancelOrder" };
  const alias = unit({ canonical_refs: refs });
  const ledger = [first, alias].sort((left, right) => left.coverage_id.localeCompare(right.coverage_id));
  assert(errorsFor(ledger).some((error) => error.includes("semantic tuple already uses coverage ID")));
});

test("requires lexicographic order", () => {
  const secondRefs = {
    entry: "zzz",
    invariant: "src/orders/state.ts#OrderStatus transitions",
    subsystem: "packages/api",
    bug_class: "BUG-CLASSES.md#State consistency",
  };
  assert(errorsFor([unit({ canonical_refs: secondRefs }), unit()])
    .some((error) => error.includes("sorted lexicographically")));
});

test("validates assignment block maps", () => {
  const overlap = unit({
    selected_companion_blocks: ["CONCURRENCY-AND-ASYNC.md#Lock held across await"],
    excluded_blocks: [{ block: "CONCURRENCY-AND-ASYNC.md#Lock held across await", reason: "Claimed irrelevant." }],
  });
  assert(errorsFor([overlap]).some((error) => error.includes("also selected")));

  const noReason = unit({ excluded_blocks: [{ block: "CONCURRENCY-AND-ASYNC.md#Lock held across await", reason: "" }] });
  assert(errorsFor([noReason]).some((error) => error.includes("reason")));

  assert.deepEqual(errorsFor([unit({ core_bug_class_block: null })]), []);
  assert(errorsFor([unit({ core_bug_class_block: " " })]).some((error) => error.includes("core_bug_class_block: expected null or non-empty text")));
});

test("requires owned artifacts for local checks and null artifacts for source checks", () => {
  const local = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [localCheck()],
  });
  assert.deepEqual(errorsFor([local]), []);

  const independentlyVerified = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts", "src/orders/state.ts"],
    local_checks: [sourceCheck(), localCheck("verifier-1", { reviewed_paths: ["src/orders/state.ts"] })],
  });
  assert.deepEqual(errorsFor([independentlyVerified]), []);

  const unownedPath = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts", "src/orders/state.ts"],
    local_checks: [sourceCheck()],
  });
  assert(errorsFor([unownedPath]).some((error) => error.includes("has no check owner")));

  for (const [checkAgentId, artifact] of [
    [null, "agents/hunter-1/artifacts/result.txt"],
    ["hunter-1", null],
    ["hunter-1", "result.txt"],
    ["hunter-1", "agents/hunter-2/artifacts/result.txt"],
    ["../hunter", "agents/../hunter/artifacts/result.txt"],
  ]) {
    const value = unit({
      status: "covered",
      agent_id: "hunter-1",
      reviewed_paths: ["src/api/orders.ts"],
      local_checks: [localCheck(checkAgentId, { artifact })],
    });
    assert.notEqual(errorsFor([value]).length, 0, `${checkAgentId}: ${artifact}`);
  }

  const unownedSource = unit({
    status: "blocked",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
    unresolved: ["The scheduler behavior is not source-visible."],
  });
  assert(errorsFor([unownedSource]).some((error) => error.includes("unit with status \"blocked\" requires a canonical lowercase agent ID")));

  const sourceWithArtifact = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck("hunter-1", { artifact: "agents/hunter-1/artifacts/source.txt" })],
  });
  assert(errorsFor([sourceWithArtifact]).some((error) => error.includes("source-only check must use null")));
});

test("requires canonical lowercase filesystem-safe agent IDs", () => {
  for (const value of ["hunter-1", "verifier_2", "a0"]) assert.equal(isSafeAgentId(value), true, value);
  for (const value of ["Hunter-1", "hunter.1", "hunter-1.", "hunter ", "con", "prn", "aux", "nul", "com1", "lpt9", "../hunter"]) {
    assert.equal(isSafeAgentId(value), false, value);
  }

  const caseAlias = unit({ status: "in_progress", agent_id: "Hunter-1" });
  assert(errorsFor([caseAlias]).some((error) => error.includes("canonical lowercase agent ID")));
});

test("enforces state evidence", () => {
  assert(errorsFor([unit({ status: "in_progress" })]).some((error) => error.includes("unit with status \"in_progress\" requires")));
  assert(errorsFor([unit({ status: "blocked" })]).some((error) => error.includes("unresolved")));
  assert(errorsFor([unit({ status: "candidate" })]).some((error) => error.includes("reviewed_paths")));

  assert.deepEqual(errorsFor([unit({ status: "in_progress", agent_id: "hunter-1" })]), []);
  const inProgressEvidence = unit({
    status: "in_progress",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
  });
  assert(errorsFor([inProgressEvidence]).some((error) => error.includes("must keep this array empty")));

  const assignedPlanned = unit({
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
  });
  assert(errorsFor([assignedPlanned]).some((error) => error.includes("planned unit must be unassigned")));

  const candidate = unit({
    status: "candidate",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck("hunter-1", { invariant: "Shipped orders stay shipped.", result: "No status guard exists." })],
    result_fingerprints: ["src-orders-cancel-after-ship"],
    unresolved: ["validation_budget_exhausted"],
  });
  assert.deepEqual(errorsFor([candidate]), []);

  const blocked = unit({
    status: "blocked",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
    unresolved: ["The production order volume is unavailable."],
  });
  assert.deepEqual(errorsFor([blocked]), []);
  const blockedFingerprint = { ...blocked, result_fingerprints: ["forbidden-fingerprint"] };
  assert(errorsFor([blockedFingerprint]).some((error) => error.includes("result_fingerprints")));

  for (const status of ["not_applicable", "out_of_scope", "deferred"]) {
    assert.deepEqual(errorsFor([unit({ status, unresolved: ["Reason recorded."] })]), []);
    const invalid = unit({
      status,
      agent_id: "hunter-1",
      reviewed_paths: ["src/api/orders.ts"],
      local_checks: [sourceCheck()],
      result_fingerprints: ["forbidden-fingerprint"],
      unresolved: ["Reason recorded."],
    });
    const errors = errorsFor([invalid]);
    assert(errors.some((error) => error.includes("must be unassigned")), status);
    assert(errors.some((error) => error.includes("reviewed_paths")), status);
    assert(errors.some((error) => error.includes("result_fingerprints")), status);
  }

  const coveredFingerprint = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
    result_fingerprints: ["forbidden-fingerprint"],
  });
  assert(errorsFor([coveredFingerprint]).some((error) => error.includes("result_fingerprints")));

  const coveredUnresolved = unit({
    status: "covered",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck()],
    unresolved: ["Unexpected unresolved claim."],
  });
  assert(errorsFor([coveredUnresolved]).some((error) => error.includes("unresolved")));
});

test("archives prior evidence when a critic assigns a fresh owner", () => {
  const reassigned = unit({
    attempts: [archivedAttempt()],
    wave: 2,
    status: "in_progress",
    agent_id: "hunter-2",
  });
  assert.deepEqual(errorsFor([reassigned]), []);

  const finalClosure = unit({
    attempts: [archivedAttempt()],
    wave: 2,
    status: "covered",
    agent_id: "hunter-2",
    reviewed_paths: ["src/orders/state.ts"],
    local_checks: [sourceCheck("hunter-2", { reviewed_paths: ["src/orders/state.ts"] })],
  });
  assert.deepEqual(errorsFor([finalClosure]), []);
});

test("preserves candidate provenance when reassignment must be deferred", () => {
  const candidateAttempt = archivedAttempt({
    status: "candidate",
    result_fingerprints: ["src-orders-cancel-after-ship"],
    unresolved: ["validation_budget_exhausted"],
  });
  const deferred = unit({
    attempts: [candidateAttempt],
    wave: 2,
    status: "deferred",
    unresolved: ["quick_profile_final_critic"],
  });
  assert.deepEqual(errorsFor([deferred]), []);
});

test("rejects reassignment owner reuse and evidence mixing", () => {
  const reusedOwner = unit({
    attempts: [archivedAttempt()],
    wave: 2,
    status: "in_progress",
    agent_id: "hunter-1",
  });
  assert(errorsFor([reusedOwner]).some((error) => error.includes("current assignment owner must be fresh")));

  const mixedEvidence = unit({
    attempts: [archivedAttempt({ local_checks: [localCheck()] })],
    wave: 2,
    status: "covered",
    agent_id: "hunter-2",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [localCheck()],
  });
  const errors = errorsFor([mixedEvidence]);
  assert(errors.some((error) => error.includes("prior assignment owner evidence must remain")));
  assert(errors.some((error) => error.includes("artifact from an archived attempt cannot be reused")));

  const mixedHistory = unit({
    attempts: [
      archivedAttempt({ local_checks: [localCheck()] }),
      archivedAttempt({
        wave: 2,
        agent_id: "hunter-2",
        local_checks: [localCheck()],
      }),
    ],
    wave: 3,
    status: "in_progress",
    agent_id: "hunter-3",
  });
  const historyErrors = errorsFor([mixedHistory]);
  assert(historyErrors.some((error) => error.includes("prior assignment owner evidence must remain in its earlier attempt")));
  assert(historyErrors.some((error) => error.includes("artifact from an earlier attempt cannot be reused")));

  const unordered = unit({
    attempts: [archivedAttempt(), archivedAttempt({
      wave: 1,
      agent_id: "hunter-2",
      local_checks: [sourceCheck("hunter-2")],
    })],
    wave: 3,
    status: "in_progress",
    agent_id: "hunter-3",
  });
  assert(errorsFor([unordered]).some((error) => error.includes("strictly increasing")));
});

test("rejects unsafe paths and malformed fingerprints", () => {
  for (const value of [
    "/etc/passwd",
    "../src/file.js",
    "src/../file.js",
    "src/con.txt",
    "src/PRN",
    "src/AUX.c",
    "src/NUL",
    "src/CLOCK$.txt",
    "src/conin$.txt",
    "src/conout$",
    "src/COM1.log",
    "src/lpt9",
    "src/COM\u00b9.log",
    "src/COM\u00b2.log",
    "src/COM\u00b3.log",
    "src/lpt\u00b9",
    "src/lpt\u00b2",
    "src/lpt\u00b3",
    "src/file.js.",
    "C:/src/file.js",
    "src\\file.js",
    "src/file\n.js",
    "src/file\u0085.js",
    "src/file\u2028.js",
    "src/file\u200b.js",
    "src/file\u034f.js",
    "src/file\ufe0f.js",
  ]) {
    assert.equal(isSafeRelativePath(value), false, value);
  }
  assert.equal(isSafeRelativePath("src/handler.js"), true);
  assert.equal(isSafeRelativePath("src/caf\u00e9/handler.js"), true);
  assert(errorsFor([unit({ starting_paths: ["../src/api/orders.ts"] })]).some((error) => error.includes("repository-relative path")));
  assert(errorsFor([unit({
    status: "candidate",
    agent_id: "hunter-1",
    reviewed_paths: ["src/api/orders.ts"],
    local_checks: [sourceCheck("hunter-1", { invariant: "Shipped orders stay shipped.", result: "No status guard exists." })],
    result_fingerprints: ["not stable"],
  })]).some((error) => error.includes("invalid fingerprint")));
});

test("rejects format, default-ignorable, and invalid-scalar prose", () => {
  for (const invisible of ["\u200b", "\u034f", "\ufe0f", "\ud800"]) {
    assert(errorsFor([unit({ entry: invisible })]).some((error) => error.includes("entry")), JSON.stringify(invisible));
    assert(errorsFor([unit({ invariant: invisible })]).some((error) => error.includes("invariant")), JSON.stringify(invisible));
    assert(errorsFor([unit({
      excluded_blocks: [{ block: "BUG-CLASSES.md#State consistency", reason: invisible }],
    })]).some((error) => error.includes("reason")), JSON.stringify(invisible));
  }
});

test("quotes input-derived controls in direct validation errors", () => {
  const invalidStatus = `invalid-${TERMINAL_CONTROL_PAYLOAD}`;
  const invalidPath = `src/${TERMINAL_CONTROL_PAYLOAD}.js`;
  const value = unit({
    status: invalidStatus,
    agent_id: "hunter-1",
    reviewed_paths: [invalidPath],
    local_checks: [sourceCheck()],
    result_fingerprints: ["force-state-error"],
  });
  const output = errorsFor([value]).join("\n");

  assert.match(output, /\$\[0\]\.status/);
  assert.match(output, /\$\[0\]\.reviewed_paths/);
  assert.match(output, /\\u001b/);
  assert.match(output, /\\u0007/);
  assert.match(output, /\\u0085/);
  assert.match(output, /\\u202e/);
  assertNoInjectedControlBytes(output);
});

test("quotes input-derived controls in CLI validation errors", () => {
  const value = unit({
    status: `invalid-${TERMINAL_CONTROL_PAYLOAD}`,
    result_fingerprints: ["force-state-error"],
  });
  const result = runCli(JSON.stringify([value]));

  assert.equal(result.status, 1, cliOutput(result));
  assert.match(result.stderr, /\$\[0\]\.status/);
  assert.match(result.stderr, /\\u001b/);
  assertNoInjectedControlBytes(result.stderr);
});

test("returns a generic syntax error without parser-supplied controls", () => {
  const malformed = Buffer.concat([
    Buffer.from("["),
    Buffer.from(TERMINAL_CONTROL_PAYLOAD),
    Buffer.from("]"),
  ]);
  const result = runCli(malformed);

  assert.equal(result.status, 1, cliOutput(result));
  assert.equal(result.stderr.replace(/\r\n/g, "\n"), "Failed to parse coverage ledger: invalid JSON syntax\n");
  assertNoInjectedControlBytes(result.stderr);
});

test("does not reflect controls from a failed CLI input path", () => {
  withTempDirectory("validate-coverage-ledger-path-", (directory) => {
    const missingPath = path.join(directory, `missing-${TERMINAL_CONTROL_PAYLOAD}.json`);
    const result = spawnSync(process.execPath, [validatorPath, missingPath], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    assert.equal(result.status, 1, cliOutput(result));
    assert.match(result.stderr, /Failed to read coverage ledger:/);
    assertNoInjectedControlBytes(result.stderr);
  });
});

test("rejects invalid UTF-8 through the CLI", () => {
  const encoded = Buffer.from(JSON.stringify([unit()]));
  const marker = Buffer.from("Cancel-order route");
  const markerOffset = encoded.indexOf(marker);
  assert.notEqual(markerOffset, -1);
  const malformed = Buffer.concat([
    encoded.subarray(0, markerOffset),
    Buffer.from([0x80]),
    encoded.subarray(markerOffset + marker.length),
  ]);

  const result = runCli(malformed);
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, /input is not valid UTF-8/);
  assert.doesNotMatch(output, /TypeError|stack|at validate-coverage-ledger/i);
});

test("rejects a FIFO through the CLI without blocking", { skip: process.platform === "win32" }, () => {
  withTempDirectory("validate-coverage-ledger-fifo-", (directory) => {
    const fifoPath = path.join(directory, "coverage-ledger.json");
    const created = spawnSync("mkfifo", [fifoPath], { encoding: "utf8", timeout: CLI_TIMEOUT_MS });
    assert.equal(created.status, 0, cliOutput(created));

    const result = spawnSync(process.execPath, [validatorPath, fifoPath], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    const output = cliOutput(result);
    assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
    assert.equal(result.status, 1, output);
    assert.match(output, /input must be a regular file/);
    assert.doesNotMatch(output, /stack|at validate-coverage-ledger/i);
  });
});

test("rejects a symlink through the CLI", (t) => {
  withTempDirectory("validate-coverage-ledger-symlink-", (directory) => {
    const targetPath = path.join(directory, "target.json");
    const symlinkPath = path.join(directory, "coverage-ledger.json");
    fs.writeFileSync(targetPath, JSON.stringify([unit()]));
    if (!trySymlink(targetPath, symlinkPath, t)) return;
    const result = spawnSync(process.execPath, [validatorPath, symlinkPath], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    const output = cliOutput(result);
    assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
    assert.equal(result.status, 1, output);
    assert.match(output, /input must not be a symlink/);
    assert.doesNotMatch(output, /stack|at validate-coverage-ledger/i);
  });
});

test("identity-checked read path accepts a regular file and rejects unsafe inputs", () => {
  withTempDirectory("validate-coverage-ledger-identity-", (directory) => {
    const ledgerPath = path.join(directory, "coverage-ledger.json");
    const contents = JSON.stringify([unit()]);
    fs.writeFileSync(ledgerPath, contents);
    assert.equal(readFileWithinLimit(ledgerPath, { identityCheck: true }), contents);

    assert.throws(
      () => readFileWithinLimit(directory, { identityCheck: true }),
      /input must be a regular file/,
    );

    const oversizedPath = path.join(directory, "oversized.json");
    fs.writeFileSync(oversizedPath, Buffer.alloc(LIMITS.inputBytes + 1, 0x20));
    assert.throws(
      () => readFileWithinLimit(oversizedPath, { identityCheck: true }),
      /input exceeds 5242880 byte limit/,
    );

    const invalidUtf8Path = path.join(directory, "invalid.json");
    fs.writeFileSync(invalidUtf8Path, Buffer.from([0x5b, 0x80, 0x5d]));
    assert.throws(
      () => readFileWithinLimit(invalidUtf8Path, { identityCheck: true }),
      /input is not valid UTF-8/,
    );
  });
});

test("identity-checked read path rejects a symlink", (t) => {
  withTempDirectory("validate-coverage-ledger-identity-link-", (directory) => {
    const targetPath = path.join(directory, "target.json");
    const symlinkPath = path.join(directory, "coverage-ledger.json");
    fs.writeFileSync(targetPath, JSON.stringify([unit()]));
    if (!trySymlink(targetPath, symlinkPath, t)) return;
    assert.throws(
      () => readFileWithinLimit(symlinkPath, { identityCheck: true }),
      /input must not be a symlink/,
    );
  });
});

test("identity-checked read path rejects a file swapped after lstat", (t) => {
  withTempDirectory("validate-coverage-ledger-identity-swap-", (directory) => {
    const ledgerPath = path.join(directory, "coverage-ledger.json");
    const decoyPath = path.join(directory, "decoy.json");
    fs.writeFileSync(ledgerPath, JSON.stringify([unit()]));
    fs.writeFileSync(decoyPath, JSON.stringify([]));
    const originalLstat = fs.lstatSync;
    t.mock.method(fs, "lstatSync", (file, options) => originalLstat.call(fs, decoyPath, options));
    assert.throws(
      () => readFileWithinLimit(ledgerPath, { identityCheck: true }),
      /input changed while it was being opened/,
    );
  });
});

test("rejects deeply nested input without recursion failure", () => {
  let nested = 0;
  for (let depth = 0; depth < 20000; depth++) nested = [nested];
  assert(errorsFor(nested).some((error) => error.includes("exceeds nesting depth limit 64")));

  const result = runCli(`${"[".repeat(20000)}0${"]".repeat(20000)}`);
  const output = cliOutput(result);
  assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
  assert.equal(result.status, 1, output);
  assert.match(output, /exceeds nesting depth limit 64/);
  assert.doesNotMatch(output, /RangeError|Maximum call stack|stack|at validate-coverage-ledger/i);
});

test("rejects multi-megabyte nesting under a constrained Node heap", () => {
  const openContainers = "[".repeat(2000000);
  const cases = [
    openContainers,
    `${openContainers}0${"]".repeat(2000000)}`,
  ];
  for (const contents of cases) {
    const result = runCli(contents, {
      nodeArgs: ["--max-old-space-size=64"],
      timeout: HOSTILE_CLI_TIMEOUT_MS,
    });
    const output = cliOutput(result);
    assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
    assert.equal(result.status, 1, output);
    assert.match(output, /exceeds nesting depth limit 64/);
    assert.doesNotMatch(output, /heap out of memory|allocation failed|RangeError|Maximum call stack|stack|at validate-coverage-ledger/i);
  }
});

test("caps malformed 10000-unit validation output", () => {
  assert.equal(errorsFor(Array.from({ length: LIMITS.units }, () => null)).length, LIMITS.validationErrors);

  const result = runCli(JSON.stringify(Array.from({ length: LIMITS.units }, () => null)));
  const output = cliOutput(result);
  assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
  assert.equal(result.status, 1, output);
  assert.match(output, /output capped at 100/);
  assert(output.length < 20000, `unexpected output length ${output.length}`);
  assert.doesNotMatch(output, /RangeError|Maximum call stack|stack|at validate-coverage-ledger/i);
});

test("rejects malformed top-level data and excessive unit counts", () => {
  assert.deepEqual(errorsFor({ units: [] }), ["$: expected a top-level array"]);
  const tooMany = Array.from({ length: 10001 }, () => null);
  const errors = errorsFor(tooMany);
  assert.deepEqual(errors, ["$: exceeds 10000 coverage units"]);

  const oversizedCollection = unit({ extra: Array.from({ length: LIMITS.collectionItems + 1 }, () => null) });
  assert(errorsFor([oversizedCollection]).some((error) => error.includes("exceeds 1000 entries")));
});

test("accepts a canonical ID derived from near-maximum multibyte references", () => {
  const canonicalRefs = {
    entry: "\u6f22".repeat(1024),
    invariant: "\u00e9".repeat(1024),
    subsystem: "packages/api",
    bug_class: "\u6f22".repeat(1023) + "\u00e9",
  };
  const value = unit({ canonical_refs: canonicalRefs });
  assert(value.coverage_id.length > 16384, `coverage_id length ${value.coverage_id.length}`);
  assert(value.coverage_id.length <= 65536, `coverage_id length ${value.coverage_id.length}`);
  assert.deepEqual(errorsFor([value]), []);

  const result = runCli(JSON.stringify([value]));
  assert.equal(result.status, 0, cliOutput(result));
});
