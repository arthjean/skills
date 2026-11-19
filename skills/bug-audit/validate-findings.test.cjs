const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");
const schema = require("./report-schema.json");
const {
  LIMITS,
  SafeInputError,
  collect,
  collectSchemaErrors,
  hasNoFollowOpen,
  isBugClassReference,
  isSameFileIdentity,
  readFileWithinLimit,
  validateDocument,
} = require("./validate-findings.cjs");

const validatorPath = path.join(__dirname, "validate-findings.cjs");
const ledgerValidatorPath = path.join(__dirname, "validate-coverage-ledger.cjs");
const CLI_TIMEOUT_MS = 5000;
const HOSTILE_CLI_TIMEOUT_MS = 15000;
const TERMINAL_CONTROL_PAYLOAD = "\u001b\u0007\u0085\u202e\u034f\ufe0f";
const TERMINAL_CONTROL_BYTES = [
  Buffer.from([0x1b]),
  Buffer.from([0x07]),
  Buffer.from("\u0085"),
  Buffer.from("\u202e"),
  Buffer.from("\u034f"),
  Buffer.from("\ufe0f"),
];

function step(kind = "entrypoint", file = "src/orders/cancel.ts", line = 10) {
  return { kind, file, line, scope: "cancelOrder", description: "The cancel request reaches the refund path." };
}

function evidence(file = "src/orders/cancel.ts", line = 10) {
  return { file, line, description: "The refund runs before the status change is committed." };
}

function confirmed() {
  return {
    verdict: "confirmed",
    fingerprint: "orders-cancel-double-refund",
    title: "Concurrent cancels refund an order twice",
    description: "Two concurrent cancel requests for the same paid order both issue a refund.",
    bug_class: "BUG-CLASSES.md#Concurrency and ordering",
    invariant: "An order is refunded at most once: OrderStatus allows CANCELED only from PAID.",
    root_cause: "cancelOrder reads the status, awaits the refund, then writes CANCELED without a conditional update.",
    trace: [step("entrypoint"), step("propagation", "src/orders/refund.ts", 20), step("failure", "src/orders/store.ts", 30)],
    evidence: [evidence()],
    conditions: [],
    reproduction: {
      trigger: "Two cancel requests for one paid order interleave at the refund await.",
      proof_kind: "deterministic_schedule",
      inputs: ["Order 42 in status PAID with a captured payment of 100."],
      instructions: ["Pause the first cancel after the status read, run the second cancel to completion, then resume the first."],
      expected_result: "One refund of 100 and status CANCELED.",
      observed_result: "Two refunds of 100 are recorded and the status is CANCELED.",
      runs: { executed: 20, failed: 20 },
      control_result: "With the two cancels serialized, one refund is recorded in 20 of 20 runs.",
    },
    regression_test: {
      file_name: "src/orders/cancel.concurrency.test.ts",
      code: "test(\"concurrent cancels refund once\", async () => { expect(await refundsAfterInterleavedCancels()).toBe(1); });",
      command: "bunx vitest run src/orders/cancel.concurrency.test.ts",
    },
    remediation: { strategy: "Make the status transition a conditional update inside the refund transaction." },
    severity: {
      likelihood: { score: "medium", reason: "Double clicks and client retries send concurrent cancels." },
      impact: { score: "high", reason: "Money leaves the account twice and needs manual recovery." },
      overall_severity: "high",
    },
    confidence: { score: "high", reason: "The schedule test is red in every run and green when serialized." },
  };
}

function needsValidation() {
  return {
    verdict: "needs_validation",
    fingerprint: "sync-cursor-skips-tied-rows",
    title: "Sync cursor may skip rows sharing a timestamp",
    description: "Pagination by updated_at may skip rows that share the boundary timestamp.",
    bug_class: "BUG-CLASSES.md#Edge cases and boundary values",
    invariant: "Every row changed since the last sync is delivered exactly once.",
    claimed_root_cause: "fetchPage filters on updated_at > cursor, so rows equal to the cursor are skipped when a page ends inside a tie.",
    trace: [step()],
    evidence: [evidence()],
    blockers: ["Whether production writes share updated_at values depends on clock precision the repository does not pin."],
    validation_plan: {
      local: "Seed two rows with the same updated_at across a page boundary and run one sync.",
      owner: "Count rows sharing an updated_at value in the production table over the last week.",
    },
  };
}

function rejected() {
  return {
    verdict: "rejected",
    fingerprint: "users-profile-stale-cache",
    title: "Stale read after profile update",
    description: "The candidate claimed profile reads return stale data after an update.",
    bug_class: "BUG-CLASSES.md#State consistency",
    invariant: "A profile read after a successful update returns the updated value.",
    claimed_root_cause: "updateProfile was claimed to skip cache invalidation.",
    trace: [step("failure")],
    evidence: [evidence()],
    reason: "updateProfile invalidates the cache key in the same commit hook before it returns.",
  };
}

function errorsFor(value) {
  return validateDocument(value, schema);
}

function withTempDirectory(prefix, callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  try {
    return callback(directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function runCli(contents, options = {}) {
  const { nodeArgs = [], timeout = CLI_TIMEOUT_MS } = options;
  return withTempDirectory("validate-findings-", (directory) => {
    const findingsPath = path.join(directory, "findings.json");
    fs.writeFileSync(findingsPath, contents);
    return spawnSync(process.execPath, [...nodeArgs, validatorPath, findingsPath], {
      encoding: "utf8",
      timeout,
    });
  });
}

function cliOutput(result) {
  return `${result.stdout}${result.stderr}`;
}

function assertNoInjectedControlBytes(output) {
  const bytes = Buffer.isBuffer(output) ? output : Buffer.from(output, "utf8");
  for (const marker of TERMINAL_CONTROL_BYTES) {
    assert.equal(bytes.indexOf(marker), -1, `found raw control bytes ${marker.toString("hex")}`);
  }
}

function trySymlink(target, link) {
  try {
    fs.symlinkSync(target, link);
    return true;
  } catch (error) {
    if (error && (error.code === "EPERM" || error.code === "EACCES")) return false;
    throw error;
  }
}

function producerShapedFindings() {
  const demonstrated = confirmed();
  demonstrated.conditions = [{
    kind: "concurrency",
    description: "Two requests for the same order run at the same time.",
  }];
  demonstrated.reproduction.inputs = ["", " \t\r\n", "\u0000\u001f\u007f", "\u034f", "\ufe0f", "\ud800", "\udc00", "[{,}]\\\""];
  demonstrated.remediation.code_changes = [{
    file_name: "src/orders/cancel.ts",
    fixed_code: "",
  }];

  const blocked = needsValidation();
  delete blocked.validation_plan.owner;

  return [demonstrated, blocked, rejected()];
}

function rejectMutation(factory, mutate) {
  const value = factory();
  mutate(value);
  assert.notEqual(errorsFor([value]).length, 0);
}

function expectError(factory, mutate, fragment) {
  const value = factory();
  mutate(value);
  const errors = errorsFor([value]);
  assert(errors.some((error) => error.includes(fragment)), `expected an error containing ${JSON.stringify(fragment)}, got ${JSON.stringify(errors)}`);
}

test("schema is an actual top-level array with exactly three branches", () => {
  assert.equal(schema.type, "array");
  assert.equal(schema.items.oneOf.length, 3);
  assert.deepEqual(schema.items.oneOf.map((branch) => branch.properties.verdict.const), [
    "confirmed", "needs_validation", "rejected",
  ]);
  const confirmedSchema = schema.items.oneOf[0].properties;
  assert.equal(confirmedSchema.title.visibleContent, true);
  assert.equal(confirmedSchema.reproduction.properties.inputs.items.minLength, undefined);
  assert.equal(confirmedSchema.reproduction.properties.inputs.items.visibleContent, undefined);
  assert.equal(confirmedSchema.remediation.properties.code_changes.items.properties.fixed_code.minLength, undefined);
  assert.deepEqual(schema.items.oneOf[0].properties.trace.items.properties.kind.enum, ["entrypoint", "propagation", "failure"]);
  assert.deepEqual(collectSchemaErrors(schema), []);
});

test("accepts a producer-shaped findings document through the CLI", () => {
  const result = runCli(JSON.stringify(producerShapedFindings()));
  assert.equal(result.status, 0, cliOutput(result));
  assert.match(result.stdout, /PASS: 3 findings valid/);
});

test("accepts empty output and each complete branch", () => {
  assert.deepEqual(errorsFor([]), []);
  assert.deepEqual(errorsFor([confirmed(), needsValidation(), rejected()]), []);
  const localOnly = needsValidation();
  delete localOnly.validation_plan.owner;
  assert.deepEqual(errorsFor([localOnly]), []);
  const ownerOnly = needsValidation();
  delete ownerOnly.validation_plan.local;
  assert.deepEqual(errorsFor([ownerOnly]), []);
  const withoutControl = confirmed();
  delete withoutControl.reproduction.control_result;
  delete withoutControl.remediation.code_changes;
  assert.deepEqual(errorsFor([withoutControl]), []);
});

test("requires a visible local or owner validation plan", () => {
  expectError(needsValidation, (finding) => { finding.validation_plan = {}; }, "requires at least one visible local or owner plan");
  rejectMutation(needsValidation, (finding) => { finding.validation_plan = { owner: " " }; });
  rejectMutation(needsValidation, (finding) => {
    finding.validation_plan = { deployment: "Check the deployed configuration." };
  });
});

test("allows one-line traces that start at an entrypoint or end at the failure", () => {
  const entryOnly = confirmed();
  entryOnly.trace = [step("entrypoint")];
  assert.deepEqual(errorsFor([entryOnly]), []);
  const failureOnly = confirmed();
  failureOnly.trace = [step("failure")];
  assert.deepEqual(errorsFor([failureOnly]), []);
});

test("enforces entrypoint, propagation, and failure trace order", () => {
  expectError(confirmed, (finding) => { finding.trace = [step("propagation")]; }, "a one-line trace must be \"entrypoint\" or \"failure\"");
  expectError(confirmed, (finding) => { finding.trace[0].kind = "failure"; }, "trace[0].kind: must be \"entrypoint\"");
  expectError(confirmed, (finding) => { finding.trace[2].kind = "propagation"; }, "trace[2].kind: must be \"failure\"");
  expectError(confirmed, (finding) => { finding.trace[1].kind = "entrypoint"; }, "trace[1].kind: must be \"propagation\"");
  expectError(rejected, (finding) => {
    finding.trace = [step("entrypoint"), step("entrypoint", "src/orders/refund.ts", 20)];
  }, "must be \"failure\"");
  rejectMutation(confirmed, (finding) => { finding.trace[2].kind = "sink"; });
});

test("rejects empty required content", () => {
  const cases = [
    [confirmed, (finding) => { finding.title = ""; }],
    [confirmed, (finding) => { finding.title = "   "; }],
    [confirmed, (finding) => { finding.invariant = ""; }],
    [confirmed, (finding) => { finding.bug_class = ""; }],
    [confirmed, (finding) => { finding.evidence = []; }],
    [confirmed, (finding) => { finding.reproduction.inputs = []; }],
    [confirmed, (finding) => { finding.reproduction.instructions = []; }],
    [confirmed, (finding) => { finding.reproduction.observed_result = ""; }],
    [confirmed, (finding) => { finding.reproduction.expected_result = ""; }],
    [confirmed, (finding) => { finding.regression_test.code = ""; }],
    [confirmed, (finding) => { finding.regression_test.command = ""; }],
    [confirmed, (finding) => { finding.remediation.strategy = ""; }],
    [confirmed, (finding) => { delete finding.regression_test; }],
    [confirmed, (finding) => { delete finding.reproduction.runs; }],
    [needsValidation, (finding) => { finding.blockers = []; }],
    [needsValidation, (finding) => { finding.invariant = ""; }],
    [needsValidation, (finding) => { finding.validation_plan = {}; }],
    [needsValidation, (finding) => { finding.validation_plan = { local: " " }; }],
    [rejected, (finding) => { finding.claimed_root_cause = ""; }],
    [rejected, (finding) => { delete finding.bug_class; }],
  ];
  for (const [factory, mutate] of cases) rejectMutation(factory, mutate);
});

test("requires a visible observed result and remediation on confirmed findings", () => {
  expectError(confirmed, (finding) => { finding.reproduction.observed_result = "\u200b"; }, "requires a visible reproduction observed_result");
  expectError(confirmed, (finding) => { delete finding.reproduction; }, "requires a visible reproduction observed_result");
  expectError(confirmed, (finding) => { finding.remediation.strategy = " "; }, "requires visible remediation");
});

test("checks reproduction run counts", () => {
  expectError(confirmed, (finding) => {
    finding.reproduction.runs = { executed: 10, failed: 11 };
  }, "reproduction.runs.failed: cannot exceed runs.executed 10");
  const partial = confirmed();
  partial.reproduction.runs = { executed: 100, failed: 37 };
  partial.reproduction.proof_kind = "stress_loop";
  assert.deepEqual(errorsFor([partial]), []);
  const single = confirmed();
  single.reproduction.runs = { executed: 1, failed: 1 };
  assert.deepEqual(errorsFor([single]), []);
  rejectMutation(confirmed, (finding) => { finding.reproduction.runs = { executed: 10, failed: 0 }; });
  rejectMutation(confirmed, (finding) => { finding.reproduction.runs = { executed: 0, failed: 0 }; });
  rejectMutation(confirmed, (finding) => { finding.reproduction.runs = { executed: 2.5, failed: 1 }; });
  rejectMutation(confirmed, (finding) => { finding.reproduction.runs = { executed: 2, failed: 1, flaky: true }; });
});

test("restricts proof kinds and condition kinds to the schema enums", () => {
  for (const kind of schema.items.oneOf[0].properties.reproduction.properties.proof_kind.enum) {
    const finding = confirmed();
    finding.reproduction.proof_kind = kind;
    assert.deepEqual(errorsFor([finding]), [], kind);
  }
  rejectMutation(confirmed, (finding) => { finding.reproduction.proof_kind = "intuition"; });
  rejectMutation(confirmed, (finding) => {
    finding.conditions = [{ kind: "authentication_level", description: "A normal account." }];
  });
});

test("requires bug_class to be a block reference on every verdict", () => {
  const valid = [
    "BUG-CLASSES.md#State consistency",
    "CONCURRENCY-AND-ASYNC.md#Lock held across await",
    "NUMERIC-TIME-AND-TEXT.md#Daylight saving transitions",
    "BUG-CLASSES.md#Core discipline (async)",
    "X.md#a",
  ];
  for (const value of valid) {
    assert.equal(isBugClassReference(value), true, value);
    for (const factory of [confirmed, needsValidation, rejected]) {
      const finding = factory();
      finding.bug_class = value;
      assert.deepEqual(errorsFor([finding]), [], value);
    }
  }

  const invalid = [
    "State consistency",
    "bug-classes.md#State consistency",
    "Bug-Classes.md#State consistency",
    "BUG-CLASSES#State consistency",
    "BUG-CLASSES.md#",
    "BUG-CLASSES.md# State consistency",
    "BUG-CLASSES.md#State consistency ",
    "BUG-CLASSES.md#State\nconsistency",
    "BUG-CLASSES.md#State\u202econsistency",
    "BUG-CLASSES.md#State\u200bconsistency",
    "BUG-CLASSES.md#State\ud800",
    "../BUG-CLASSES.md#State consistency",
    "docs/BUG-CLASSES.md#State consistency",
    "BUG_CLASSES.md#State consistency",
    "1BUG.md#State",
  ];
  for (const value of invalid) {
    assert.equal(isBugClassReference(value), false, JSON.stringify(value));
    for (const factory of [confirmed, needsValidation, rejected]) {
      const finding = factory();
      finding.bug_class = value;
      assert.notEqual(errorsFor([finding]).length, 0, JSON.stringify(value));
    }
  }
  expectError(confirmed, (finding) => { finding.bug_class = "State consistency"; }, "bug_class: must be a block reference");
});

test("rejects fields that belong to another verdict", () => {
  const cases = [
    [confirmed, "claimed_root_cause", "The claimed cause."],
    [confirmed, "blockers", ["A blocker."]],
    [confirmed, "validation_plan", { local: "A plan." }],
    [confirmed, "reason", "A reason."],
    [needsValidation, "severity", confirmed().severity],
    [needsValidation, "reproduction", confirmed().reproduction],
    [needsValidation, "regression_test", confirmed().regression_test],
    [needsValidation, "remediation", confirmed().remediation],
    [needsValidation, "reason", "A reason."],
    [needsValidation, "root_cause", "A cause."],
    [needsValidation, "confidence", confirmed().confidence],
    [rejected, "severity", confirmed().severity],
    [rejected, "reproduction", confirmed().reproduction],
    [rejected, "regression_test", confirmed().regression_test],
    [rejected, "remediation", confirmed().remediation],
    [rejected, "blockers", ["A blocker."]],
    [rejected, "validation_plan", { local: "A plan." }],
    [rejected, "root_cause", "A cause."],
    [rejected, "confidence", confirmed().confidence],
  ];
  for (const [factory, field, value] of cases) {
    const verdict = factory().verdict;
    expectError(factory, (finding) => { finding[field] = value; }, `${verdict} finding must not contain "${field}"`);
  }
});

test("preserves exact input and replacement-code strings", () => {
  const finding = confirmed();
  const inputs = ["", " \t\r\n", "\u0000\u001f\u007f", "\u034f", "\ufe0f", "\ud800", "\udc00"];
  const fixedCode = "\u0000 \t\r\n\u001f\u007f\u034f\ufe0f\ud800x\udc00";
  finding.reproduction.inputs = inputs.slice();
  finding.remediation.code_changes = [{ file_name: "src/orders/cancel.ts", fixed_code: fixedCode }];

  assert.deepEqual(errorsFor([finding]), []);
  assert.deepEqual(finding.reproduction.inputs, inputs);
  assert.equal(finding.remediation.code_changes[0].fixed_code, fixedCode);
});

test("rejects invalid scalars and whitespace, control, format, or default-ignorable prose", () => {
  for (const invisible of ["\u0000\t\r\n\u001f\u007f\u200b", "\u034f", "\ufe0f", "\ud800", "\udc00", "visible\ud800"]) {
    const cases = [
      [confirmed, (finding) => { finding.title = invisible; }],
      [confirmed, (finding) => { finding.invariant = invisible; }],
      [confirmed, (finding) => { finding.trace[0].scope = invisible; }],
      [confirmed, (finding) => { finding.evidence[0].description = invisible; }],
      [confirmed, (finding) => { finding.reproduction.instructions = [invisible]; }],
      [confirmed, (finding) => { finding.reproduction.trigger = invisible; }],
      [confirmed, (finding) => { finding.reproduction.control_result = invisible; }],
      [confirmed, (finding) => { finding.regression_test.command = invisible; }],
      [confirmed, (finding) => { finding.remediation.strategy = invisible; }],
      [confirmed, (finding) => { finding.severity.impact.reason = invisible; }],
      [confirmed, (finding) => { finding.confidence.reason = invisible; }],
      [needsValidation, (finding) => { finding.blockers = [invisible]; }],
      [needsValidation, (finding) => { finding.validation_plan = { local: invisible }; }],
      [needsValidation, (finding) => { finding.validation_plan = { owner: invisible }; }],
      [rejected, (finding) => { finding.reason = invisible; }],
    ];
    for (const [factory, mutate] of cases) rejectMutation(factory, mutate);
  }
});

test("quotes input-derived controls in direct validation values and paths", () => {
  const finding = confirmed();
  finding.trace[0].kind = `invalid-${TERMINAL_CONTROL_PAYLOAD}`;
  finding.reproduction[`extra-${TERMINAL_CONTROL_PAYLOAD}`] = "value";
  finding.bug_class = `BUG-CLASSES.md#State${TERMINAL_CONTROL_PAYLOAD}`;

  const cyclic = {};
  cyclic[`path-${TERMINAL_CONTROL_PAYLOAD}`] = cyclic;
  const output = [
    ...errorsFor([finding]),
    ...collect(cyclic, { type: "object" }, "$input"),
  ].join("\n");

  for (const escaped of ["\\u001b", "\\u0007", "\\u0085", "\\u202e", "\\u034f", "\\ufe0f"]) {
    assert(output.includes(escaped), `missing escaped diagnostic ${escaped}`);
  }
  assertNoInjectedControlBytes(output);
});

test("rejects line zero", () => {
  rejectMutation(confirmed, (finding) => { finding.trace[0].line = 0; });
  rejectMutation(rejected, (finding) => { finding.evidence[0].line = 0; });
});

test("does not treat inherited or Object-prototype properties as schema properties", () => {
  rejectMutation(confirmed, (finding) => { finding.constructor = "not allowed"; });
  const inherited = Object.create({ verdict: "confirmed" });
  assert(errorsFor([inherited]).some((error) => error.includes("exactly one")));
  assert(collect(Object.create({ constructor: "inherited" }), {
    type: "object",
    properties: { constructor: { type: "string" } },
    required: ["constructor"],
    additionalProperties: false,
  }).some((error) => error.includes("missing required")));
});

test("oneOf requires exactly one passing branch", () => {
  assert(collect("value", { oneOf: [{ type: "string" }, { minLength: 1 }] }, "$test")
    .some((error) => error.includes("matched 2")));
  assert(collect(7, { oneOf: [{ type: "string" }, { minimum: 10 }] }, "$test")
    .some((error) => error.includes("matched 0")));
});

test("rejects duplicate fingerprints and unique array entries", () => {
  const first = confirmed();
  const second = rejected();
  second.fingerprint = first.fingerprint;
  assert(errorsFor([first, second]).some((error) => error.includes("duplicate of")));
  rejectMutation(needsValidation, (finding) => { finding.blockers = [finding.blockers[0], finding.blockers[0]]; });
  rejectMutation(confirmed, (finding) => { finding.reproduction.inputs = ["same", "same"]; });
});

test("uses canonical Set uniqueness for structured entries at the array limit", () => {
  const entries = Array.from({ length: LIMITS.arrayItems }, (_, id) => ({ id, label: String(id) }));
  assert.deepEqual(collect(entries, { type: "array", uniqueItems: true }), []);

  const duplicate = entries.slice(0, -1);
  duplicate.push({ label: "0", id: 0 });
  assert(collect(duplicate, { type: "array", uniqueItems: true })
    .some((error) => error.includes(`duplicate at index ${LIMITS.arrayItems - 1}`)));
});

test("bounds canonical uniqueness keys and Set storage", () => {
  const oversizedKey = "x".repeat(LIMITS.canonicalKeyBytes + 1);
  assert(collect([oversizedKey], { type: "array", uniqueItems: true })
    .some((error) => error.includes("canonical key exceeds")));

  const itemLength = Math.floor(LIMITS.uniqueSetBytes / 6);
  const largeUniqueItems = Array.from({ length: 6 }, (_, index) => `${index}${"x".repeat(itemLength)}`);
  assert(collect(largeUniqueItems, { type: "array", uniqueItems: true })
    .some((error) => error.includes("canonical uniqueness set exceeds")));
});

test("requires findings to be sorted by fingerprint", () => {
  const first = confirmed();
  const second = rejected();
  assert(errorsFor([second, first]).some((error) => error.includes("sorted lexicographically")));
});

test("rejects severity above demonstrated impact", () => {
  expectError(confirmed, (finding) => {
    finding.severity.overall_severity = "critical";
    finding.severity.impact.score = "high";
  }, "cannot exceed demonstrated impact");
});

test("rejects unsafe source paths", () => {
  const badPaths = [
    "/etc/passwd",
    "../src/file.c",
    "src/../file.c",
    "src//file.c",
    "C:\\src\\file.c",
    "src/file:name.c",
    "src/file\nname.c",
    "src/file\u0001name.c",
    "src/file\u0085name.c",
    "src/file\u2028name.c",
    "src/file\u202ename.c",
    "src/file\u2066name.c",
    "src/file\u200dname.c",
    "src/file\u034fname.c",
    "src/file\ufe0fname.c",
    "src/file\ud800name.c",
    "src/file\udc00name.c",
    "CON",
    "src/con.txt",
    "src/PRN",
    "src/AUX.c",
    "src/NUL",
    "src/COM1.log",
    "src/lpt9",
    "src/CONIN$",
    "src/CONOUT$.txt",
    "src/CLOCK$.txt",
    "src/COM\u00b9.log",
    "src/LPT\u00b2.log",
    "src /file.c",
    "src./file.c",
    "src/file.c ",
    "src/file.c.",
  ];
  for (const badPath of badPaths) {
    rejectMutation(confirmed, (finding) => { finding.trace[0].file = badPath; });
    rejectMutation(confirmed, (finding) => { finding.regression_test.file_name = badPath; });
  }
  rejectMutation(rejected, (finding) => { finding.evidence[0].file = "NUL.txt"; });
  rejectMutation(confirmed, (finding) => {
    finding.remediation.code_changes = [{ file_name: "src/file:name.c", fixed_code: "replacement" }];
  });
  expectError(confirmed, (finding) => { finding.regression_test.file_name = "../outside.test.ts"; }, "regression_test.file_name: must be a safe repository-relative source path");
});

test("accepts legitimate Unicode source paths and prose", () => {
  const finding = confirmed();
  finding.title = "Finding \ud83d\ude00 cafe\u0301";
  finding.trace[0].file = "src/日本語/cafe\u0301-\ud83d\ude00.ts";
  finding.evidence[0].file = "src/mañana/файл.ts";
  finding.regression_test.file_name = "tests/修正/éxito.test.ts";
  finding.remediation.code_changes = [{
    file_name: "src/修正/éxito.ts",
    fixed_code: "replacement",
  }];
  assert.deepEqual(errorsFor([finding]), []);
});

test("CLI rejects input above the byte limit without an exception trace", () => {
  const result = runCli(Buffer.alloc(LIMITS.inputBytes + 1, 0x20));
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, new RegExp(`input exceeds ${LIMITS.inputBytes} byte limit`));
  assert.doesNotMatch(output, /RangeError|Maximum call stack|heap out of memory/i);
});

test("CLI rejects invalid UTF-8 without replacement or an exception trace", () => {
  const findings = producerShapedFindings();
  findings[0].reproduction.inputs = ["INVALID_UTF8"];
  const encoded = Buffer.from(JSON.stringify(findings));
  const marker = Buffer.from("INVALID_UTF8");
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
  assert.doesNotMatch(output, /TypeError|stack|at validate-findings/i);
});

test("quotes input-derived controls in CLI validation errors", () => {
  const finding = confirmed();
  finding.trace[0].kind = `invalid-${TERMINAL_CONTROL_PAYLOAD}`;
  finding.reproduction[`extra-${TERMINAL_CONTROL_PAYLOAD}`] = "value";
  const result = runCli(JSON.stringify([finding]));

  assert.equal(result.status, 1, cliOutput(result));
  assert.match(result.stderr, /\$\[0\]\.trace\[0\]\.kind/);
  assert.match(result.stderr, /\\u001b/);
  assert.match(result.stderr, /\\u202e/);
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
  assert.equal(result.stderr, "Failed to parse findings JSON: invalid JSON syntax\n");
  assertNoInjectedControlBytes(result.stderr);
});

test("does not reflect controls from a failed CLI input path", () => {
  withTempDirectory("validate-findings-path-", (directory) => {
    const missingPath = path.join(directory, `missing-${TERMINAL_CONTROL_PAYLOAD}.json`);
    const result = spawnSync(process.execPath, [validatorPath, missingPath], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    assert.equal(result.status, 1, cliOutput(result));
    assert.match(result.stderr, /Failed to read findings JSON:/);
    assertNoInjectedControlBytes(result.stderr);
  });
});

test("CLI rejects lone-surrogate prose without changing input semantics", () => {
  const findings = producerShapedFindings();
  findings[0].title = "\ud800";
  const result = runCli(JSON.stringify(findings));
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, /must contain only valid Unicode scalar values/);
  assert.doesNotMatch(output, /stack|at validate-findings/i);
});

test("CLI rejects Unicode format controls in source paths", () => {
  const findings = producerShapedFindings();
  findings[0].trace[0].file = "src/file\u202ename.c";
  const result = runCli(JSON.stringify(findings));
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, /must be a safe repository-relative source path/);
  assert.doesNotMatch(output, /stack|at validate-findings/i);
});

test("CLI rejects a directory input", () => {
  withTempDirectory("validate-findings-directory-", (directory) => {
    const result = spawnSync(process.execPath, [validatorPath, directory], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    const output = cliOutput(result);
    assert.equal(result.status, 1, output);
    assert.match(output, /Failed to read findings JSON:/);
    assert.doesNotMatch(output, /stack|at validate-findings/i);
  });
});

test("CLI rejects a FIFO without blocking", { skip: process.platform === "win32" || !hasNoFollowOpen() }, () => {
  withTempDirectory("validate-findings-fifo-", (directory) => {
    const fifoPath = path.join(directory, "findings.json");
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
    assert.doesNotMatch(output, /stack|at validate-findings/i);
  });
});

test("CLI rejects a symlink without following it", (t) => {
  withTempDirectory("validate-findings-symlink-", (directory) => {
    const targetPath = path.join(directory, "target.json");
    const symlinkPath = path.join(directory, "findings.json");
    fs.writeFileSync(targetPath, JSON.stringify(producerShapedFindings()));
    if (!trySymlink(targetPath, symlinkPath)) {
      t.skip("symlink creation is not permitted on this machine");
      return;
    }
    const result = spawnSync(process.execPath, [validatorPath, symlinkPath], {
      encoding: "utf8",
      timeout: CLI_TIMEOUT_MS,
    });
    const output = cliOutput(result);
    assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
    assert.equal(result.status, 1, output);
    assert.match(output, /input must not be a symlink/);
    assert.doesNotMatch(output, /stack|at validate-findings/i);
  });
});

test("identity-checked read accepts a regular file and returns its exact text", () => {
  withTempDirectory("validate-findings-identity-", (directory) => {
    const filePath = path.join(directory, "findings.json");
    const contents = JSON.stringify(producerShapedFindings());
    fs.writeFileSync(filePath, contents);
    assert.equal(readFileWithinLimit(filePath, true), contents);
    assert.equal(readFileWithinLimit(filePath), contents);
  });
});

test("identity-checked read rejects directories, oversized files, and invalid UTF-8", () => {
  withTempDirectory("validate-findings-identity-", (directory) => {
    assert.throws(() => readFileWithinLimit(directory, true), (error) =>
      error instanceof SafeInputError && /input must be a regular file/.test(error.message));

    const oversized = path.join(directory, "oversized.json");
    fs.writeFileSync(oversized, Buffer.alloc(LIMITS.inputBytes + 1, 0x20));
    assert.throws(() => readFileWithinLimit(oversized, true), (error) =>
      error instanceof SafeInputError && new RegExp(`input exceeds ${LIMITS.inputBytes} byte limit`).test(error.message));

    const malformed = path.join(directory, "malformed.json");
    fs.writeFileSync(malformed, Buffer.from([0x5b, 0x80, 0x5d]));
    assert.throws(() => readFileWithinLimit(malformed, true), (error) =>
      error instanceof SafeInputError && /input is not valid UTF-8/.test(error.message));

    assert.throws(() => readFileWithinLimit(path.join(directory, "missing.json"), true), (error) =>
      !(error instanceof SafeInputError) && error.code === "ENOENT");
  });
});

test("identity-checked read rejects a symlink", (t) => {
  withTempDirectory("validate-findings-identity-", (directory) => {
    const targetPath = path.join(directory, "target.json");
    const symlinkPath = path.join(directory, "findings.json");
    fs.writeFileSync(targetPath, "[]");
    if (!trySymlink(targetPath, symlinkPath)) {
      t.skip("symlink creation is not permitted on this machine");
      return;
    }
    assert.throws(() => readFileWithinLimit(symlinkPath, true), (error) =>
      error instanceof SafeInputError && /input must not be a symlink/.test(error.message));
  });
});

test("identity comparison requires the same device and inode", () => {
  assert.equal(isSameFileIdentity({ dev: 1n, ino: 2n }, { dev: 1n, ino: 2n }), true);
  assert.equal(isSameFileIdentity({ dev: 1n, ino: 2n }, { dev: 1n, ino: 3n }), false);
  assert.equal(isSameFileIdentity({ dev: 1n, ino: 2n }, { dev: 4n, ino: 2n }), false);
  assert.equal(isSameFileIdentity(null, { dev: 1n, ino: 2n }), false);
  withTempDirectory("validate-findings-identity-", (directory) => {
    const first = path.join(directory, "first.json");
    const second = path.join(directory, "second.json");
    fs.writeFileSync(first, "[]");
    fs.writeFileSync(second, "[]");
    const firstStat = fs.lstatSync(first, { bigint: true });
    const descriptor = fs.openSync(second, "r");
    try {
      assert.equal(isSameFileIdentity(firstStat, fs.fstatSync(descriptor, { bigint: true })), false);
    } finally {
      fs.closeSync(descriptor);
    }
    const sameDescriptor = fs.openSync(first, "r");
    try {
      assert.equal(isSameFileIdentity(firstStat, fs.fstatSync(sameDescriptor, { bigint: true })), true);
    } finally {
      fs.closeSync(sameDescriptor);
    }
  });
});

test("uses the identity-checked read by default only when no-follow open is unavailable", () => {
  const expected = !(Number.isInteger(fs.constants.O_NOFOLLOW) && fs.constants.O_NOFOLLOW !== 0 &&
    Number.isInteger(fs.constants.O_NONBLOCK) && fs.constants.O_NONBLOCK !== 0);
  assert.equal(!hasNoFollowOpen(), expected);
  if (process.platform === "win32") assert.equal(hasNoFollowOpen(), false);
});

test("CLI rejects input above the nesting-depth limit without an exception trace", () => {
  const levels = LIMITS.nestingDepth + 1;
  const result = runCli(`${"[".repeat(levels)}0${"]".repeat(levels)}`);
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, new RegExp(`${LIMITS.nestingDepth} level nesting depth limit`));
  assert.doesNotMatch(output, /RangeError|Maximum call stack|heap out of memory/i);
});

test("CLI rejects an oversized array without an exception trace", () => {
  const result = runCli(JSON.stringify(Array(LIMITS.arrayItems + 1).fill(null)));
  const output = cliOutput(result);
  assert.equal(result.status, 1, output);
  assert.match(output, new RegExp(`${LIMITS.arrayItems} item array limit`));
  assert.doesNotMatch(output, /RangeError|Maximum call stack|heap out of memory/i);
});

test("checks pattern and branch invariants", () => {
  rejectMutation(rejected, (finding) => { finding.fingerprint = "not stable"; });
  rejectMutation(needsValidation, (finding) => {
    finding.severity = { impact: { score: "low" }, overall_severity: "low" };
  });
});

test("rejects unsupported and malformed schema keywords", () => {
  assert(collectSchemaErrors({ type: "string", format: "uuid" }).some((error) => error.includes("format")));
  assert(collectSchemaErrors({ type: "string", pattern: "[" }).some((error) => error.includes("regular expression")));
  assert(collectSchemaErrors({ type: "string", visibleContent: "yes" }).some((error) => error.includes("expected boolean")));
  assert(collectSchemaErrors({ type: "array", visibleContent: true }).some((error) => error.includes("requires type")));
  assert.notEqual(validateDocument([], { type: "array", maxItems: 1 }).length, 0);
});

test("caps malformed 1000-finding validation output", () => {
  assert.equal(errorsFor(Array.from({ length: LIMITS.arrayItems }, () => null)).length, LIMITS.validationErrors);

  const result = runCli(JSON.stringify(Array.from({ length: LIMITS.arrayItems }, () => null)));
  const output = cliOutput(result);
  assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
  assert.equal(result.status, 1, output);
  assert.match(output, /output capped at 100/);
  assert(output.length < 20000, `unexpected output length ${output.length}`);
  assert.doesNotMatch(output, /RangeError|Maximum call stack|stack|at validate-findings/i);
});

test("caps amplified in-limit findings output under a constrained Node heap", () => {
  const findings = Array.from({ length: 750 }, () => ({
    verdict: "confirmed",
    trace: Array.from({ length: LIMITS.arrayItems }, () => 0),
    evidence: Array.from({ length: LIMITS.arrayItems }, () => 0),
  }));
  const contents = JSON.stringify(findings);
  assert(contents.length > 3 * 1000 * 1000, `hostile input too small: ${contents.length}`);
  assert(contents.length <= LIMITS.inputBytes, `hostile input over limit: ${contents.length}`);

  const result = runCli(contents, {
    nodeArgs: ["--max-old-space-size=64"],
    timeout: HOSTILE_CLI_TIMEOUT_MS,
  });
  const output = cliOutput(result);
  assert.notEqual(result.error && result.error.code, "ETIMEDOUT", output);
  assert.equal(result.status, 1, output);
  assert.match(output, /output capped at 100/);
  assert(output.length < 20000, `unexpected output length ${output.length}`);
  assert.doesNotMatch(output, /heap out of memory|allocation failed|RangeError|Maximum call stack/i);
});

test("keeps shared helpers aligned with the coverage-ledger validator", { skip: !fs.existsSync(ledgerValidatorPath) }, () => {
  const findingsModule = require("./validate-findings.cjs");
  const ledgerModule = require("./validate-coverage-ledger.cjs");

  for (const name of [
    "VISIBLE_CONTENT",
    "PATH_FORBIDDEN_CHARACTER",
    "WINDOWS_RESERVED_COMPONENT",
    "UNSAFE_DIAGNOSTIC_CHARACTER",
  ]) {
    assert.equal(findingsModule[name].source, ledgerModule[name].source, `${name} source`);
    assert.equal(findingsModule[name].flags, ledgerModule[name].flags, `${name} flags`);
  }

  const sharedLimitKeys = Object.keys(findingsModule.LIMITS)
    .filter((key) => Object.prototype.hasOwnProperty.call(ledgerModule.LIMITS, key))
    .sort();
  assert.deepEqual(sharedLimitKeys, ["inputBytes", "nestingDepth", "validationErrors"]);
  for (const key of sharedLimitKeys) {
    assert.equal(findingsModule.LIMITS[key], ledgerModule.LIMITS[key], `LIMITS.${key}`);
  }

  const pathCorpus = [
    "src/handler.js",
    "src/caf\u00e9/handler.js",
    "src/\u65e5\u672c\u8a9e/\u0444\u0430\u0439\u043b.ts",
    "src/cloc\u212a$.txt",
    "src/CLOCK$.txt",
    "src/con.txt",
    "CON",
    "src/COM\u00b9.log",
    "src/lpt\u00b3",
    "/etc/passwd",
    "../src/file.c",
    "src/../file.c",
    "src//file.c",
    "src\\file.c",
    "src/file:name.c",
    "~home/file.c",
    "C:/file.c",
    "src/file.c ",
    "src/file.c.",
    "src/file\u202ename.c",
    "src/file\u200b.js",
    "src/file\u034f.js",
    "src/file\ufe0f.js",
    "src/file\ud800name.c",
    "src/file\udc00name.c",
  ];
  for (const value of pathCorpus) {
    assert.equal(
      findingsModule.isSafeRelativeSourcePath(value),
      ledgerModule.isSafeRelativePath(value),
      `path verdict diverges for ${JSON.stringify(value)}`,
    );
  }
  assert.equal(findingsModule.isSafeRelativeSourcePath("src/cloc\u212a$.txt"), false);
  assert.equal(ledgerModule.isSafeRelativePath("src/cloc\u212a$.txt"), false);

  const proseCorpus = [
    "Valid prose.",
    "caf\u00e9",
    "",
    " \t\r\n",
    "\u200b",
    "\u034f",
    "\ufe0f",
    "\ud800",
    "\udc00",
    "visible\ud800",
  ];
  for (const value of proseCorpus) {
    assert.equal(
      findingsModule.hasVisibleProse(value),
      ledgerModule.hasVisibleProse(value),
      `prose verdict diverges for ${JSON.stringify(value)}`,
    );
  }
});
