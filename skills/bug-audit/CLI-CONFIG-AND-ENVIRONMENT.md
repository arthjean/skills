# CLI, Configuration, and Environment Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds code whose correctness depends on its process contract or its environment: command-line tools and scripts, build or deployment helpers, daemons started from the shell, programs that read configuration files, environment variables, or flags, and code that writes, replaces, or walks files and paths. Select it for a source-visible argument parser, config loader, exit path, signal handler, or filesystem write, not for the presence of a `bin` entry alone.

Use alongside `BUG-CLASSES.md`. This file covers the process contract, filesystem semantics, and configuration loading. Use `NATIVE-AND-SYSTEMS.md` for raw descriptors, syscalls, and platform APIs below the language runtime, `CONCURRENCY-AND-ASYNC.md` for concurrent writers inside one process, and `DATA-AND-PERSISTENCE.md` when the file is a store with its own durability contract.

## Core discipline (include in every agent prompt for this domain)

```
- Treat the process contract as the interface: arguments, environment, stdin, stdout, stderr, exit status, signals, and the files the program creates or replaces. Name which part of that contract breaks and who relies on it (a script, CI, a pipeline, another program).
- Drive the real program as a subprocess in a scratch directory with fixture files and an explicit, minimal environment, then assert on exit status, both output streams, and the resulting files.
- Test the environment axes the code depends on: platform path rules, line endings, case sensitivity, working directory, locale, and whether stdout is a terminal or a pipe.
- Interrupt and fail deliberately: kill the process at a chosen point, close its output early, make a write fail, and observe what remains on disk.
- Use `confirmed` for a red subprocess or unit check on the local platform. Use `needs_validation` when the decisive behavior belongs to another operating system, a user's shell, or a deployed environment value not visible in source.
```

## Process contract bug classes (subagent_type: `general`)

**Exit status that misreports the outcome**
The program exits 0 after a failure (error logged then normal return, a rejected promise not awaited before exit, a catch-all handler that prints and continues), exits non-zero on success, or loses the child's status in a wrapper script (`|| true`, a pipeline without `pipefail`, `set -e` disabled inside a function called from a condition). Callers and CI then act on a false result. Prove with a harness that runs the command on a failing fixture and asserts the exit status, with a control fixture that succeeds.

**Output stream mixing and loss**
Machine-readable output on stdout is polluted by logs, progress bars, or warnings; errors go to stdout so pipelines consume them as data; color codes are emitted when stdout is not a terminal; or output written just before an explicit process exit is lost because the stream was still buffered or asynchronous. Prove with a harness that pipes stdout into a parser and asserts it parses and is complete, with stderr captured separately.

**Signal and interruption handling**
An interrupt or termination signal leaves temporary files, lock files, half-written outputs, or orphaned child processes; cleanup runs twice; or the program ignores the signal and must be killed. A second interrupt during cleanup may corrupt state further. Prove with fault_injection: send the signal at a deterministic point (after a marker line is printed or a hook fires) and assert on the files and processes left behind.

**Broken pipe and closed input**
Writing to a closed pipe (output piped to `head`) raises an error the program reports as a crash or a non-zero failure; reading from a closed or empty stdin hangs or treats end of input as data. Prove with a harness that pipes into a consumer that exits early, or feeds empty and unterminated stdin, and asserts status and output.

## Filesystem bug classes (subagent_type: `general`)

**Partial writes and non-atomic replacement**
A file is rewritten in place, so a crash, a full disk, or a concurrent reader observes a truncated or mixed file. Atomic replacement needs a temporary file in the same directory followed by a rename; a rename across filesystems is a copy, and on Windows replacing a file another process holds open can fail. Prove with fault_injection: fail or kill the write midway and assert the original content is still intact.

**Path handling across platforms**
Paths are built with string concatenation and a hard-coded separator, compared case-sensitively on a case-insensitive filesystem, assume no drive letter, break on spaces or non-ASCII names, hit reserved Windows names (`CON`, `NUL`) or trailing dots and spaces, or exceed path length limits. Prove with a unit_test that runs the path logic through the platform-specific variant (for example Node's `path.win32` and `path.posix`), or a harness on fixture names with spaces, Unicode, and mixed case.

**Working directory and implicit file assumptions**
Relative paths resolve against the current working directory instead of the program or config location; temporary files are created in a fixed name and collide between runs; text is read with the platform default encoding or line ending; executable bits or symlinks are assumed where the platform lacks them. Prove with a harness that runs the same command from two working directories, or two concurrent invocations, and asserts identical results.

## Configuration bug classes (subagent_type: `general`)

**Config precedence and merge errors**
Flags, environment variables, config files, and defaults are combined in an order that differs from the documented one; a deep merge replaces arrays instead of merging them, or merges them where replacement was intended; a later source silently overrides an explicit flag. Prove with a property_test over generated combinations of sources asserting the documented precedence, or a unit_test with the specific conflicting combination.

**Silent defaults and unknown keys**
A misspelled key is ignored and the default applies; a missing file, section, or variable falls back to a value that is valid in development and wrong elsewhere; a zero or empty value is replaced by the default because falsy and absent are conflated. Prove with a unit_test that loads a config with a typo or an explicit zero and asserts the effective value.

**Environment variable parsing**
Strings like `"false"`, `"0"`, or `"no"` are truthy; empty and unset are treated the same or differently from what the docs say; numbers with whitespace or units parse to NaN or a partial value; lists split on inconsistent separators. Prove with a unit_test over the parsed values for each spelling the docs and callers use.

**Argument parsing edges**
Option values that begin with `-`, the `--` terminator, repeated flags (last wins versus accumulate), negative numbers, flags combined into one token, and quoted arguments with spaces are parsed differently from the help text. Prove with a property_test or table-driven unit_test that runs each argument vector through the real parser and compares with the documented meaning.

## Universal moves (apply across the above)

- Write the process contract table: every input channel, every output channel, exit statuses and their meanings, signals handled, and files created, replaced, or deleted.
- Run each command with stdout as a pipe and as a file, with empty stdin, from another working directory, and with each documented environment variable unset, empty, and set.
- Kill or fail the process at each write, then inspect the filesystem.
- Diff the effective configuration against the documented precedence for every source pair.

## Validation rules (apply before reporting ANY finding here)

1. Name the broken part of the process contract or configuration rule, the exact invocation, environment, and files that trigger it, and the consumer that relies on the correct behavior.
2. Confirm no visible layer already handles it: a wrapper that sets `pipefail`, a config schema that rejects unknown keys, an atomic-write helper, or a signal handler registered elsewhere.
3. A `confirmed` record needs a red subprocess or unit check on the local platform with the environment written out explicitly (variable names and safe values only), plus a control invocation that behaves correctly where practical.
4. For interruption and partial-write defects, show the corrupted or leftover state after a deterministic interruption point, not after a random kill.
5. Use `needs_validation` when the defect requires another operating system, a specific shell, or a deployed environment value, and name the owner-observed check (run this invocation on that platform, print the effective config on the host).
