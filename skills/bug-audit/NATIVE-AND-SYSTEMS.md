# Native and Systems Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds native or systems-level code: Rust, C, C++, Go, or Zig modules that manage memory, handles, or processes directly; `unsafe` blocks; FFI bindings (C ABI, cgo, JNI, Python or Node native extensions); direct use of files, sockets, pipes, signals, child processes, or memory-mapped files; or a product shipped to several operating systems or architectures (a CI matrix, cross-compilation targets, platform-conditional code). A compiled language alone is not enough. Select it when source crosses an ownership boundary, depends on integer width or build profile, or calls an OS interface whose semantics differ by platform.

Use `NUMERIC-TIME-AND-TEXT.md` for floating point, decimal, time, and text semantics, `CONCURRENCY-AND-ASYNC.md` for threads and async scheduling, `DATA-AND-PERSISTENCE.md` for files used as a data store, and `CLI-CONFIG-AND-ENVIRONMENT.md` for argument parsing, configuration, and environment variables. This file owns representation, ownership and lifetime, unwinding, OS interfaces, and platform differences.

## Core discipline (include in every agent prompt for this domain)

```
- Check the build profile the project ships. Release and debug differ: Rust disables overflow checks in the release profile by default, C and C++ remove `assert` under `NDEBUG`, and optimization exposes undefined behavior that debug builds hide. Prove the defect in the shipped profile or state the difference explicitly.
- Undefined behavior is a defect even when tests pass. Prove it with a sanitizer (AddressSanitizer, UndefinedBehaviorSanitizer, ThreadSanitizer, MemorySanitizer) or Miri for Rust, and name the exact operation reported.
- For every allocation, handle, or callback that crosses a boundary, name the allocator, the owner, the release point, and the thread that may use it.
- For platform defects, run on the affected platform when it is available locally, or reproduce the platform rule in a `unit_test` or `differential` (path normalization, case folding, line endings). Otherwise use `needs_validation` naming the platform.
- Use `needs_validation` for target architectures, OS versions, filesystems, or hardware that cannot be exercised locally.
```

## Integer and representation bug classes (subagent_type: `general`)

**Silent integer wrap and truncation**
Rust release builds wrap on overflow unless overflow checks are enabled; `as` casts truncate and reinterpret silently (`u64 as u32`, `-1i32 as usize`, and float-to-integer casts saturate); C and C++ signed overflow is undefined while unsigned arithmetic wraps; Go integers wrap silently; JavaScript bitwise operators coerce to 32-bit integers. Typical defects: a length subtraction that underflows, a count multiplied past the type maximum, millisecond timestamps stored in 32 bits, identifiers truncated on narrowing. Proof: `unit_test` or `property_test` at the type boundaries, run in the shipped profile; `sanitizer` (UBSan) for C and C++.

**Platform data-model differences**
C `long` is 32 bits on 64-bit Windows and 64 bits on 64-bit Linux and macOS; `usize` is 32 bits on 32-bit and wasm32 targets; signed and unsigned size types are mixed at API boundaries; binary formats are read in native byte order; struct padding leaks into serialized bytes or hashes. Proof: `unit_test` with explicit-width fixtures and byte-order assertions, or a `differential` on a second target when a cross toolchain is already installed; otherwise `needs_validation` naming the target.

## Ownership and lifetime bug classes (subagent_type: `general`)

**Destructor and cleanup ordering**
Rust struct fields drop in declaration order, so a guard can outlive or predecease the resource it protects; a temporary guard lives to the end of its enclosing statement or block; `std::process::exit`, `abort`, `mem::forget`, and `panic = "abort"` skip destructors, leaving buffers unflushed and temporary files behind; C++ static initialization and destruction order across translation units is unspecified; Go `defer` inside a loop runs when the function returns, not per iteration, so files stay open. Proof: `unit_test` with drop-tracking types asserting the observable effect (flush, file removed, lock released), or `leak_measurement` over iterations for deferred cleanup.

**FFI ownership and allocator mismatch**
Memory allocated on one side is freed by the other side's allocator; `CString::into_raw` is never reclaimed with `from_raw`, or reclaimed twice; C retains a pointer into Rust- or Go-owned memory after the call returns (cgo's pointer-passing rules forbid this); a callback is invoked after the closure it captures was dropped; a length means bytes on one side and elements on the other; a buffer is assumed to be NUL-terminated. Proof: `sanitizer` (AddressSanitizer for leaks, double frees, and use after free), Miri for the Rust side, or `leak_measurement` over repeated calls.

**Unwinding across boundaries**
A Rust panic or C++ exception unwinds through `extern "C"` frames or C library callbacks, which aborts or is undefined depending on language and toolchain version; a panic inside `Drop` during unwinding aborts the process; an FFI entry point lacks `catch_unwind` (or the equivalent) and turns a recoverable error into a crash of the host process. Proof: `harness` that drives the failing path across the boundary and records whether the process aborts or the caller receives an error code.

**Unsafe invariants and aliasing**
An `unsafe impl Send` or `Sync` covers a type with non-thread-safe interior state; `Vec::set_len` runs before the elements are initialized; two `&mut` references alias; `transmute` assumes a layout the compiler does not guarantee; `get_unchecked` relies on a bound established elsewhere that a later change broke. Go `append` to a sub-slice with remaining capacity overwrites the parent slice's elements through the shared backing array. C++ iterators are invalidated by `push_back`, references bind to temporaries, and a `string_view` outlives its string. Proof: `sanitizer`, Miri, or a `unit_test` that observes the corrupted value.

## Operating-system interface bug classes (subagent_type: `general`)

**Short reads, short writes, and interrupted calls**
A read or write that returns fewer bytes than requested is treated as complete (Rust `write` instead of `write_all`, `read` instead of `read_exact`, Go code that ignores `n` when `Read` returns data and an error together, C `send` on a socket); `EINTR` is not retried; a non-blocking `EAGAIN` is treated as end of file or a fatal error; writing to a closed pipe raises `SIGPIPE` or `EPIPE` and the program crashes or loses the error. Proof: `fault_injection` with a reader or writer wrapper that returns one byte at a time or interrupts calls, and a `harness` that closes the pipe mid-write.

**Process, signal, and child handling**
Child processes are never waited on (zombies) or survive the parent on some platforms; a signal handler calls functions that are not async-signal-safe; a multithreaded process calls `fork` and the child touches locks held by threads that no longer exist; exit codes are lost through a shell wrapper or a pipeline; command lines are assembled as one string and split differently by the Windows argument parser than by a POSIX shell. Proof: `harness` that spawns, signals, and reaps children and asserts the exit status and argument vector the child actually received.

**Filesystem semantics across platforms**
Case-insensitive filesystems (the default on Windows NTFS and macOS APFS) make `Report.txt` and `report.txt` the same file; Windows uses backslash separators and drive letters, reserves names such as `CON`, `NUL`, and `COM1`, strips trailing dots and spaces, limits path length unless long paths are enabled, and refuses by default to delete or rename over a file another handle holds open; `core.autocrlf` turns line endings into CRLF and breaks parsers, hashes, and snapshot tests; `rename` across volumes fails with `EXDEV`; timestamp resolution differs by filesystem; symlinks and Unix permission bits may be absent. Proof: `unit_test` with platform-rule fixtures (case collisions, reserved names, separator handling, CRLF input), or a `differential` on the affected platform when available; otherwise `needs_validation` naming the platform and filesystem.

## Universal moves (apply across the above)

- Build an ownership table at every native boundary: allocator, owner, release point, thread affinity, and unwinding behavior. Most native defects are one cell where the two sides disagree.
- Run the relevant tests in the shipped build profile and again under the sanitizers or Miri the toolchain already provides; a debug-only green run proves little about release behavior.
- Derive the supported platforms from the CI matrix, build targets, and platform-conditional code, then grep for platform-dependent operations: paths built by string concatenation, hard-coded separators, `\n` assumptions, case-sensitive name comparisons, and fixed-width integer casts.

## Validation rules (apply before reporting ANY finding here)

1. Name the operation, the representation, ownership, or OS contract it breaks, the build profile and platform, and the observable wrong result: wrapped value, corrupted or leaked memory, aborted process, lost data, or a path that resolves to the wrong file.
2. `confirmed` requires a red check in the shipped profile (or a stated, decisive profile difference) with a control, such as the boundary value minus one, the checked arithmetic, or the correctly paired release, that passes.
3. A sanitizer or Miri report is a proof of undefined behavior only when a realistic input reaches the reported operation; show the path from an entry point.
4. Check source-visible guards before reporting: checked arithmetic, explicit-width types, `catch_unwind` at the boundary, platform-specific code paths, or a path library that normalizes separators and case. If one covers every real path, reject.
5. Return `needs_validation` when the defect requires an architecture, OS version, filesystem, or hardware unavailable locally, and give the bounded cross-target test or owner-observed platform check that would decide it.
