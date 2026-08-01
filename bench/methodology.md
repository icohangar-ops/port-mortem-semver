# Benchmark methodology

```
node bench/run.js        # CLI comparison, writes bench/results.json
cargo bench --bench compare   # in-process Criterion microbenchmarks
make bench                    # both
```

## What is compared

The original `vendor/node-semver/bin/semver.js` running on Node, against the
Rust `target/release/semver`. Both are driven through the same CLI surface with
the same arguments, so this measures the thing a user actually invokes rather
than a hand-picked library entry point. Every throughput scenario also asserts
that the two binaries produced **byte-identical stdout**; a benchmark where the
two sides disagree is not a benchmark.

Two numbers are reported, because a port can easily win one and lose the other.

### Startup

One process per invocation, 30 timed runs after one untimed warm-up, reported as
the median (means and min/max land in `results.json`). This is how the `semver`
CLI is used from shell scripts, `package.json` scripts and CI, and it is
dominated by fixed costs — V8 boot and CommonJS module loading on one side,
process exec and regex compilation on the other — not by semver itself.

Three scenarios cover the fixed costs that differ: `version-check` (parse only),
`compare` (parse and sort), and `range-filter` (the range machinery, which
compiles considerably more of the regex table).

### Throughput

The CLI sorts every version it is handed and filters by range, so passing 20,000
versions in a single invocation exercises the library 20,000 times in one
process. The same scenario is then timed with a *single* version and that median
is subtracted, which removes runtime boot and one-off setup from both sides and
leaves something close to library time.

This is deliberately conservative: the subtraction is the only correction
applied, and any per-version argv handling or output formatting is still counted
against both implementations.

## Caveats

- Wall-clock timings on a developer laptop; absolute numbers are not portable
  between machines, but the ratios have been stable across runs.
- 20,000 arguments is a large argv. Both sides pay for it, but it does put a
  floor under the measured work.
- The Criterion benchmark in `bench/compare.rs` measures the Rust library
  in-process, with no CLI or process overhead at all. Use it for optimization
  work; use this one for "what would a user notice".

## Findings

The Rust port is faster on both axes, but getting there required two fixes that
the startup benchmark is what exposed. Both are in `src/re.rs`, and neither
changes behaviour — the differential fuzzer stayed at zero divergences across
millions of calls through all of it.

Startup for a single version check, measured as this benchmark was iterated on:

| | median |
| --- | --- |
| Node original | ~46ms |
| Rust, first measurement | ~590ms |
| Rust, lazy regex compilation | ~22ms |
| Rust, ASCII digit classes | ~22ms |
| Rust, capped lazy-DFA budget | ~2ms |

1. **All 86 regexes were compiled eagerly** the first time any of them was
   touched, so every invocation paid for the entire token table. They are now
   compiled individually on first use, which is what took a version check from
   590ms to 22ms. A range operation still touches ten or so of them.

2. **`\d` was compiling as the Unicode decimal-number class.** JavaScript's `\d`
   (without the `u` flag) is exactly `[0-9]`, so this was also a small
   faithfulness bug. It matters for compile time because the ReDoS-hardened
   patterns repeat `\d` up to 256 times and a Unicode class costs dozens of
   ranges per repetition: compiling the whole table went from 556ms to 24ms.

3. **The lazy DFA budget was the real range-parsing cost.** Those same bounded
   repetitions give the hybrid regex engine a huge state space, and with a 16MB
   cache it spent ~160ms building it out before doing useful work. Versions and
   ranges are short strings, so the DFA never pays for itself; capping the cache
   at 256KB makes the engine fall back to the NFA searchers and took range
   filtering from ~180ms to ~6ms, with no measurable effect on throughput.

Current results are in `results.json`. On the machine used for development:
startup is roughly 23x faster for version operations and 8x for range
operations, and per-version throughput is roughly 2x for sorting and 10x for
range satisfaction.
