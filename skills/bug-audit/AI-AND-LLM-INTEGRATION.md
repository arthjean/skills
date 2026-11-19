# AI and LLM Integration Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds code that calls a language model and acts on its output: prompt or message assembly, structured output or JSON parsing of completions, streaming handlers, tool or function calling loops, agent orchestration, retrieval that fills a context window, conversation history management, and model or provider configuration. Select it for source-visible model calls and the code that consumes their results, not for the presence of an SDK in the manifest.

Use alongside `BUG-CLASSES.md`. This file covers model output handling, streaming and context assembly, and agent and tool execution. Use `DISTRIBUTED-AND-MESSAGING.md` for generic retry, timeout, and idempotency mechanics around any remote call, `CONCURRENCY-AND-ASYNC.md` for parallel tool execution inside one process, and `NUMERIC-TIME-AND-TEXT.md` for byte and character accounting outside model token limits.

## Core discipline (include in every agent prompt for this domain)

```
- Treat the model as an unreliable dependency: every completion can be truncated, malformed, refused, empty, differently ordered, or stopped for a reason the code ignores. Name which of these outputs breaks the code and what the user or system then gets wrong.
- Never call a paid or live model API. Replace the client at the SDK seam with a fake that replays recorded or synthetic outputs, including the ugly ones: cut-off JSON, extra prose around JSON, a refusal, a max-token stop, a tool call with a missing argument, an empty stream.
- Drive the real parser, handler, and loop code with those outputs and assert on the resulting state, side effects, and user-visible result.
- Count side effects through fakes: every tool execution, write, message, or charge a turn performs, under success, retry, and partial failure.
- Use `confirmed` for a red replay or fault-injection check. Use `needs_validation` when the decisive fact is how the live provider or model actually behaves (which stop reasons it emits, how often it truncates, a production prompt size), and name the recorded trace or log an owner can inspect.
```

## Model output handling bug classes (subagent_type: `general`)

**Truncated or malformed structured output**
The code assumes the completion is complete, valid JSON or the expected schema, but a token limit cuts it mid-object, the model wraps it in prose or code fences, or a field has the wrong type. The parser throws an unhandled error, or a lenient fallback returns an empty or default object that downstream code treats as a real answer. Prove with a harness that replays truncated, fenced, and wrong-typed outputs through the real parse path and asserts on what the caller receives.

**Ignored stop reason**
The code reads the content without checking why generation stopped: a max-token stop is treated as a finished answer, a tool-use stop is rendered as text, a refusal or content-filter stop becomes an empty success, or a stop sequence trims a required suffix. Prove with a harness that replays each stop reason the provider documents and asserts the code takes a distinct, correct path for each.

**Schema drift between tool definitions and handlers**
The schema sent to the model and the handler that executes the call disagree: a field optional in the schema is required by the handler, an enum lists values the handler does not accept, a parameter was renamed in one place, or the handler trusts types the schema does not constrain. Prove with a property_test that generates arguments valid under the declared schema and feeds them to the handler, asserting it accepts every one.

## Streaming and context bug classes (subagent_type: `general`)

**Streaming chunk boundary errors**
The streaming handler assumes chunks align with meaningful units: a multi-byte character, an event line, or a JSON fragment split across reads is decoded or parsed per chunk; tool-call argument deltas are concatenated in arrival order across calls instead of by call index; the final chunk carrying usage or the stop reason is dropped. Prove with a property_test that splits a recorded stream at every byte offset and asserts the reassembled text, tool calls, and stop reason are identical to the unsplit run.

**Context overflow and silent truncation**
History trimming, retrieval packing, or document chunking exceeds the context window, or stays under it by silently dropping content: the system instruction, the latest user message, a tool result the model must see, or the middle of a document with no marker. Token counts estimated from characters drift on non-English text and code. Prove with a unit_test that assembles the context for an oversized input and asserts which required parts survive, using the project's own token counter or a deterministic stand-in.

**Prompt and message assembly errors**
Template variables render as `undefined`, `None`, or an empty string; messages are ordered or attributed to the wrong role; a retry appends the same messages twice; few-shot examples or tool results are interleaved in an order the provider rejects. Prove with a unit_test that snapshots the assembled request for a representative turn and for a retried turn, and asserts the exact message list.

## Agent loop and tool execution bug classes (subagent_type: `general`)

**Retries that duplicate tool side effects**
A whole turn is retried after a timeout or parse failure, re-executing tools that already wrote data, sent a message, or charged a cost, because tool calls carry no idempotency key and the loop does not record completed calls. Prove with fault_injection: fail the model call after the first tool executes, let the loop retry, and count executions on a fake tool, with a control where no failure occurs.

**Tool result correlation errors**
Results are matched to calls by position instead of call ID, parallel calls are assumed to return in request order, a failed call has no result message, or a result is attached to the wrong turn, so the model reasons over mismatched data. Prove with a deterministic_schedule that completes parallel fake tools in reverse order and asserts each result reaches the matching call ID.

**Error results reported as success**
A tool exception is stringified into a normal result, an empty result stands in for a failure, or a partial batch is reported as complete, so the model and the user are told an action succeeded. Prove with fault_injection at the tool seam and assert on the message sent back to the model and the final user-visible outcome.

**Non-terminating or runaway agent loops**
The loop has no iteration or budget ceiling, repeats the same failing tool call, or never reaches a terminal state when the model keeps requesting tools; cost and latency grow without bound and the user sees no result. Prove with a harness whose fake model always requests the same tool, asserting the loop stops within its declared bound.

## Testing and configuration bug classes (subagent_type: `general`)

**Model variability hidden by fixed mocks**
Tests mock one canned, well-formed response, so every untested output shape (reordered keys, extra fields, different casing in enums, an extra sentence before the JSON) is unexercised and fails in production. This is a finding only when a reachable output shape breaks the code. Prove with a property_test or fuzz over output variations the schema or provider permits, run against the real parser.

**Model and parameter configuration mismatch**
The configured model ID, parameters, or response format disagree with what the code assumes: a parameter the chosen model does not support, a hard-coded context size that differs from the model in use, a response-format flag the parsing code does not expect, or two call sites using different models for one contract. Prove with a unit_test on the configuration resolution and the request builder; whether the live provider accepts a parameter is `needs_validation` unless the provider's documentation is cited.

## Universal moves (apply across the above)

- Build an output table per model call: every stop reason, output shape, and failure the provider documents, and the code path each one takes.
- Replace the client with a recording fake; replay real traces when the repository has them, and synthesize the edge outputs when it does not.
- Count every side effect per turn under success, retry, timeout, and parse failure.
- Assemble the context for the largest realistic input and list what was dropped.

## Validation rules (apply before reporting ANY finding here)

1. Name the model output or provider behavior that triggers the defect, the code path it takes, and the wrong result: a crash, a false success, a duplicated side effect, lost context, or a loop that does not stop.
2. Confirm no visible layer already handles it: SDK-level retries with idempotency, a schema-validating parser with a repair step, a framework that correlates tool calls by ID, or a loop guard defined elsewhere.
3. A `confirmed` record needs a red replay, fault-injection, or property check that runs entirely against fakes, with a control output that passes where practical. Never call a paid or live model API to prove it.
4. A shape the provider's documentation rules out is not a trigger unless the code targets several providers or models; cite the documentation when relying on it.
5. Use `needs_validation` when the decisive fact is live model or provider behavior, production prompt sizes, or deployed model configuration, and name the owner-observed check (a query over logged stop reasons, a sample of recorded traces, the deployed model ID).
