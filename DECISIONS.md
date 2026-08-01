# DECISIONS.md

Every non-trivial architectural divergence from `npm/node-semver@6e05b763`, with rationale.
Empty bullets do not count — each entry records what changed, why, and how we verified it.

## 1. Safe Rust only — no `unsafe`, no FFI into Node

**Divergence.** The entire library and both binaries are `#![forbid(unsafe_code)]`. The Node test adapter talks to Rust over a JSON-lines RPC child process, not via N-API / Neon / raw FFI.

**Why.** Track F's Zero Unsafe bonus and Code Quality criterion both penalize escape hatches. An N-API binding would couple the port to the Node ABI and invite `unsafe`. A child-process bridge keeps the artifact a pure Rust binary while still letting the original suite drive it.

**Verified.** `rg '\bunsafe\b' src` is empty; clippy clean; 9,182/9,182 original assertions pass through the bridge.

## 2. Faithful npm semantics, not Cargo's `semver` crate

**Divergence.** We did **not** depend on crates.io `semver` (dtolnay) or any npm-compatible `node-semver` crate. Parsing, caret/tilde/x-range desugaring, loose mode, and `includePrerelease` are ported from the JavaScript source.

**Why.** Cargo SemVer rejects constructs npm accepts (`v1.2.3`, `=1.2.3`, loose forms) and treats ranges differently. Reusing a pre-existing port would also violate Port Mortem's "no pre-existing ports" rule.

**Verified.** Upstream fixtures + 3.4M differential calls against vendored JS agree on results and error text.

## 3. Identifier representation: `Numeric(u64)` / `Alpha(String)`

**Divergence.** JavaScript numberifies prerelease/build identifiers that match `/^[0-9]+$/` and fit in `Number.MAX_SAFE_INTEGER`. Rust uses an explicit enum instead of `serde_json::Value` or a stringly API.

**Why.** Preserves npm's "numeric identifiers compare as numbers, and numbers sort before non-numbers" rule without inventing a third type system. `u64` covers the full JS safe-integer range.

**Verified.** `test/fixtures/comparisons.js` and `test/fixtures/equality.js` pass unmodified; identifier fuzz ops match.

## 4. JavaScript float stringification for huge range bounds

**Divergence.** When caret/x-range expansion does `+M + 1` on a major that exceeds JS float precision, the original emits scientific notation in the desugared comparator (e.g. `<1e+25.0.0-0`). We reproduce that stringification instead of using big integers.

**Why.** Behavioral equivalence includes the *failure mode*. A bigint-correct expansion would accept or reject different inputs than npm and fail differential tests.

**Verified.** Unit test `huge_majors_follow_js_float_stringification`; differential suite includes overflow probes.

## 5. `sort` / `rsort` error text follows V8's comparison order

**Divergence.** When a list contains an invalid version, JS throws from inside `Array#sort`, whose first comparison is typically `(list[1], list[0])`. We match which invalid version the error names, not a left-to-right scan.

**Why.** Original tests and consumers that stringify errors would diverge on message text even when ordering of valid elements is identical.

**Verified.** Differential fuzz compares exact error strings; adapter `sort`/`rsort` use JS `Array#sort` over Rust `compareBuild` for the suite path.

## 6. `simplify` swallows invalid ranges (via `satisfies`)

**Divergence.** Early port constructed a `Range` up front and errored on invalid input. The original routes through `satisfies`, which catches range errors, so an invalid range matches nothing and simplify returns accordingly.

**Why.** Caught by differential testing against the real package. Matching the silent-failure semantics is required for suite parity.

**Verified.** `ranges_api::tests::simplify_tolerates_an_invalid_range`; zero fuzz divergences on `simplifyRange`.

## 7. Regex engine: ASCII `\d`, lazy compile, capped lazy-DFA

**Divergence.** Three interlocking choices in `src/re.rs`:
1. Compile each of the 86 tokens on first use (not all eagerly).
2. Emit `[0-9]` wherever JS `\d` appears (without the `u` flag).
3. Cap the regex crate's lazy-DFA cache at 256KB so short strings use NFA searchers.

**Why.** Eager compile made CLI startup ~590ms (worse than Node). Unicode `\d` was both a faithfulness bug and a compile-time cost on ReDoS-hardened `{0,256}` repetitions. A 16MB DFA cache spent ~160ms building state that never paid off for SemVer-length inputs.

**Verified.** Startup: 590ms → 2.0ms (version check); fuzzer stayed at 0 divergences across all three fixes; suite 2.4× faster as a side effect.

## 8. Synchronous FIFO RPC for the thin adapter

**Divergence.** `adapter/bridge/rpc.js` redirects the `semver-rpc` child's stdio through a pair of FIFOs and uses blocking `fs.readSync`/`writeSync`, instead of async pipes or per-call `spawn`.

**Why.** node-semver's API is synchronous. Async pipes deadlock if the event loop is blocked waiting; per-call spawn is too slow for ~9k assertions. FIFOs give ~9µs/call synchronous RPC without `unsafe` or N-API.

**Verified.** Full original suite in ~14s; documented in `adapter/README.md`.

## 9. Object identity stays in JavaScript; decisions stay in Rust

**Divergence.** `SemVer` / `Comparator` / `Range` are real JS classes so `instanceof` and property order match. `Range.set` memoization and cross-object `set` swaps remain in JS. All parsing, desugaring, comparison, and satisfaction decisions are Rust.

**Why.** Several original tests assert reference equality of comparator arrays and mutate `range.set` between instances. Those are language/object-model concerns, not SemVer semantics. Pushing them through JSON would invent a worse protocol.

**Verified.** `test/classes/range.js` and `test/ranges/subset.js` pass; every semver decision still differential-fuzzed against vendored JS.

## 10. `typeof` guards raised in JavaScript

**Divergence.** TypeErrors like `Invalid version. Must be a string. Got type "undefined".` are thrown in the adapter before the RPC call.

**Why.** JSON cannot distinguish `undefined` from `null`. Without JS-side guards, `null` would be reported as `"object"` and fail tests that check exact TypeError text.

**Verified.** `test/functions/parse.js` and related type-guard tests pass unmodified.

## 11. CLI argv parsing hand-rolled (not clap for the public binary)

**Divergence.** `src/bin/semver.rs` mirrors `bin/semver.js`'s hand-rolled switch (including `--flag=value` splitting and the `-i` errant-value warning). Clap remains a Cargo dependency for flexibility but the public CLI path does not rely on clap's opinionated parsing.

**Why.** Snapshot tests in `test/bin/semver.js` lock stdout, stderr, and exit codes. Clap's help text and option greediness diverge from npm's CLI.

**Verified.** All CLI tap snapshots pass against `target/release/semver`.

## 12. Thread-local parse-range memo instead of a true LRU

**Divergence.** JS uses a small LRU (`internal/lrucache.js`). Rust uses a thread-local `HashMap` that clears at 1,000 entries.

**Why.** Semantically transparent for a deterministic pure function; avoids `Mutex` on the hot path and keeps the library `Send`/`Sync`-friendly without `unsafe`. Cache policy is not part of the public contract.

**Verified.** No differential divergence; suite pass rate unaffected.

## 13. Adapter copies tests at `pretest` rather than symlinking

**Divergence.** `adapter/test` is a verbatim mirror of `tests/original/`, regenerated by `scripts/sync-tests.js`, not a symlink.

**Why.** Node realpath-resolves modules before `require`, so symlinked tests look for `../../functions/` next to `tests/original/`. `--preserve-symlinks-main` does not fix child processes spawned by `debug` / CLI tests.

**Verified.** Kickoff hashes in `tests/original.SHA256SUMS` still match; pretest only copies, never edits the hashed originals.

## 14. Coverage enforcement disabled in the adapter package

**Divergence.** Original `tap` config enforces 100% coverage of its JavaScript sources. Adapter turns that off.

**Why.** The covered surface is now Rust. Enforcing JS coverage would score the bridge plumbing, not the port. `test/map.js` (every source file has a matching test file) still runs and passes.

**Verified.** Documented in `adapter/README.md`; suite green without coverage gate.

## Bug notes (differential catches)

These are not divergences — they are defects the rewrite process surfaced in *our* early port (and one JS float quirk we chose to preserve). Filing upstream against npm is optional; the float stringification is intentional JS behavior, not a bug we claim for Bug Catcher.

| Finding | Where | Disposition |
| --- | --- | --- |
| `simplify` rejected invalid ranges | our port | fixed to match JS |
| `sort` named wrong invalid version | our port | fixed to match V8 order |
| Huge majors need float stringification | JS behavior | preserved deliberately |
| Eager regex compile / Unicode `\d` / DFA budget | our port | fixed; 0 fuzz divergence retained |
