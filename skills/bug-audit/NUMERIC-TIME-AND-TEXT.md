# Numeric, Time, and Text Bug Hunting

#### When to use this file

Reach for this file when reconnaissance finds code whose correctness depends on representation rather than control flow: money, quantities, rates, or scores computed in code; unit conversions; aggregation and rounding; timestamps, durations, schedules, expiries, calendars, or recurring events; and text that is measured, truncated, compared, sorted, normalized, parsed, or formatted for a locale. Select it for the computation or conversion found in source, not for the presence of a date or decimal library.

Use alongside `BUG-CLASSES.md`. This file covers numeric representation, time and calendar semantics, and text encoding and locale. Use `DATA-AND-PERSISTENCE.md` when the defect is in a column type, a database time zone setting, or a serialization round trip through a store, and `FRONTEND-AND-UI-STATE.md` when the wrong value only appears through client state.

## Core discipline (include in every agent prompt for this domain)

```
- State the exact value that goes wrong: the input, the computed result, the expected result, and the unit or zone each is in. "Floats are imprecise" is not a finding; "the invoice total for these three lines is off by one cent" is.
- Find the representation at every handoff: integer width, float, decimal scale, string; UTC instant, local wall time, date-only; bytes, UTF-16 code units, code points, graphemes. Most defects live where two representations meet.
- Pin the environment that drives the result: time zone, locale, clock, and rounding mode. A defect that depends on one of them sets the triggering value explicitly in the check and records it.
- Property tests are the default proof here: round-trip, sum preservation, monotonicity, and idempotence over generated values find counterexamples readers miss. Record the shrunk counterexample and the seed.
- Use `confirmed` for a red check with the triggering value pinned. Use `needs_validation` when the decisive fact is production data shape, a deployed time zone or locale, or an external system's rounding or calendar rules not visible in source.
```

## Numeric representation bug classes (subagent_type: `general`)

**Binary floating point for exact quantities**
Money, inventory, percentages that must sum to 100, or any quantity compared for equality is held in binary floating point, so sums drift, equality checks fail, and `0.1 + 0.2` style errors reach totals. Prove with a property_test that splits an amount into generated parts and asserts the parts sum back to the exact total, or a unit_test with the minimal counterexample.

**Rounding mode and rounding point**
The code rounds with a mode the domain does not use (Python `round` rounds half to even, JavaScript `Math.round` rounds half toward positive infinity, `toFixed` works on the binary value so `(1.005).toFixed(2)` is `"1.00"`), rounds per line instead of per total or the reverse, or rounds twice. Prove with a unit_test on the boundary values (x.5, negative halves, values just below a half) and a differential against the rule the domain documents.

**Integer division, truncation, and sign**
Division and remainder semantics differ across languages: Python `//` and `%` floor toward negative infinity, while C, Java, Go, and Rust truncate toward zero, so negative inputs bucket, page, or wrap differently. Integer division before multiplication drops precision (`a / b * c`). Prove with a property_test that includes negative and zero operands, or a differential between the two implementations of the same formula.

**Overflow and precision loss across types**
A value crosses a narrower or lossy type: 64-bit IDs parsed as JavaScript numbers beyond `Number.MAX_SAFE_INTEGER`, JSON big integers rounded by a parser, a sum accumulated in 32 bits, a cast from float to int that saturates or wraps, a currency converted to minor units with the wrong exponent. Prove with a unit_test at the type limit and one past it, asserting the round trip.

## Time and calendar bug classes (subagent_type: `general`)

**Time zone and DST transitions**
Local wall times that do not exist (spring-forward gap) or occur twice (fall-back overlap) are created, scheduled, or compared; "add one day" is implemented as "add 24 hours"; a recurring job runs twice or not at all on transition days. Prove with a unit_test pinned to a zone with DST and dates on both transition days, asserting the scheduled instants.

**Wall clock used for durations**
Elapsed time, timeouts, rate windows, or ordering are computed from wall-clock time, which jumps on NTP correction, manual change, or VM resume, so durations go negative, timeouts fire early, or events sort out of order. Prove with a unit_test that injects a clock moving backward between two readings; the control uses a monotonic source.

**Date-only values shifted by a zone**
A calendar date (birthday, due date, billing day) is stored or parsed as an instant at midnight, then rendered in another zone as the previous or next day. In JavaScript, `new Date("2024-03-10")` parses as UTC while `new Date("2024-03-10T00:00")` parses as local time. Prove with a unit_test that formats the date under two pinned zones on either side of UTC.

**Calendar arithmetic edges**
Month-end and leap-day arithmetic (January 31 plus one month, February 29 plus one year), week numbering across years, quarter and fiscal boundaries, end-of-day as 23:59:59 dropping the last second or leap second handling, and inclusive versus exclusive range ends. Prove with a property_test over generated dates checking an invariant (adding then subtracting returns the original, ranges partition without gaps or overlaps), or unit_tests on the named edges.

## Text and encoding bug classes (subagent_type: `general`)

**Length and slicing in the wrong unit**
Limits, truncation, cursor positions, or offsets are computed in one unit and applied in another: JavaScript `length` counts UTF-16 code units, Python `len` counts code points, Rust `str::len` and Go `len` count bytes, and users perceive graphemes. Slicing can split a surrogate pair, a combining sequence, or an emoji with modifiers, and byte limits in a database or protocol reject strings that passed a character check. Prove with a unit_test using emoji with skin-tone modifiers, combining marks, and non-Latin scripts at the limit.

**Normalization and case folding mismatches**
Equal-looking strings compare unequal because one is NFC and the other NFD, case-insensitive matching uses lowercasing instead of case folding, or case mapping is locale-sensitive (Turkish dotted and dotless i), so lookups, deduplication, and uniqueness checks disagree with the store. Prove with a unit_test pairing the composed and decomposed forms, or the Turkish locale, through the real comparison path.

**Locale-dependent parsing and formatting**
Parsing or formatting depends on the process or user locale: decimal comma versus point, thousands separators, date order, collation for sorting, or number formatting that is then parsed back. Results change between developer machines, servers, and users. Prove with a differential that runs the same parse and format round trip under two pinned locales.

**Encoding round-trip loss**
Text is decoded with the wrong charset, invalid bytes are silently replaced, a byte order mark leaks into the first field, line endings change between platforms, or a multi-byte character is split across buffer or chunk boundaries. Prove with a fuzz or property_test that round-trips generated text through the encode and decode path, splitting input at every offset where chunking is involved.

## Universal moves (apply across the above)

- Write a representation table for each computed value: type, unit, scale, zone, encoding, and every conversion on the way to storage and display.
- Generate values at the edges: zero, negative, half, limit, limit plus one, transition days, February 29, month ends, multi-unit characters, decomposed forms.
- Run the same check under two pinned time zones and two locales; a result that changes is a candidate.
- Compare the code's rule with the domain's documented rule (tax, billing, scheduling spec) and cite where the domain rule comes from.

## Validation rules (apply before reporting ANY finding here)

1. Give the exact input, the observed value, the expected value, and the source of the expected rule (a spec, a doc, a test, a caller assumption). Imprecision with no consumer that depends on exactness is a hardening note.
2. Confirm no visible layer normalizes the value first: a decimal type at the boundary, a database column with fixed scale, a formatting library with an explicit locale, or an earlier normalization step.
3. A `confirmed` record needs a red check with the triggering zone, locale, clock, or value pinned and recorded, plus a control value that passes where practical.
4. For property tests, record the seed and the shrunk counterexample, and keep the counterexample as a fixed regression case.
5. Use `needs_validation` when correctness depends on production data distribution, a deployed zone or locale setting, or an external system's rounding or calendar rule, and name the owner-observed check (a data query for affected rows, the configured zone of the host, the counterpart's documented rule).
