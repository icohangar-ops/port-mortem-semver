# Node adapter

This package makes the **unmodified** npm/node-semver test suite run against the
Rust port. It exposes the same module layout and the same public API as
`semver@7.8.5`, but every semver decision is made by Rust.

```
npm install
npm test        # runs tests/original/ against the Rust port
```

The Rust binaries must exist first (`cargo build --release` from the repository
root, or `make build`). Override their location with `SEMVER_RPC_BIN` and
`SEMVER_CLI_BIN`.

## How it works

`bridge/rpc.js` starts **one** `semver-rpc` child process per Node process and
talks to it in JSON lines.

The interesting problem is that node-semver's API is synchronous — `new
SemVer('1.2.3')` has to return a fully parsed object before the next statement
runs — so the bridge cannot use Node's asynchronous child-process streams:
blocking the event loop to wait for a reply would deadlock, and spawning a
process per call would be unusably slow. Instead the child's stdin and stdout
are redirected through a pair of FIFOs. `fs.openSync` opens a FIFO in *blocking*
mode, so `fs.readSync` parks the thread until Rust answers. That gives real
synchronous request/response at roughly **9µs per call**, fast enough to run the
suite's ~9,000 assertions (hundreds of thousands of calls) in about 14 seconds.

Errors cross the bridge with the constructor name Rust's `SemverError` says
JavaScript would have used, so `TypeError` vs `Error` is preserved.

## What lives where

| Layer | Where the logic is |
| --- | --- |
| Version and range parsing, comparison, increment, coercion, subset, intersection, simplification | **Rust** |
| `SemVer` / `Comparator` / `Range` object shape, `instanceof`, property order, lazy `formatted` cache, comparator memoization | JavaScript |
| `typeof` guards and the resulting `TypeError` text | JavaScript |

Two decisions are worth spelling out.

**Type errors are raised in JavaScript.** `typeof` is a language-level concept,
and JSON cannot distinguish `undefined` from `null`, so a `null` version would
otherwise be reported as `Got type "object"` when the original said
`"undefined"`. The guards live in `bridge/args.js` and mirror the original's
checks exactly; everything that survives them is a string and goes to Rust.

**Object plumbing stays in JavaScript.** `Range`'s constructor still splits on
`||`, filters empty comparator lists and drops null sets, because the tests
inspect and even mutate `range.set` (`test/classes/range.js` asserts that two
identical ranges share the very same comparator array, and `test/ranges/subset.js`
swaps `set` between range objects). All the actual desugaring — hyphen ranges,
`~`, `^`, x-ranges, `>=0.0.0`, loose filtering, dedupe — happens in Rust behind
the `parseRangeSet` op, and the array it returns is memoized in the same LRU
cache the original uses, keyed the same way.

## Modules reproduced rather than bridged

These are under `internal/` and are copied verbatim from
`vendor/node-semver/internal/`:

- `re.js` — the regex token table. Rust builds byte-identical patterns from the
  same `createToken` sequence, so the tokens, indices and capture groups already
  line up; copying the module is what makes `semver.re`, `semver.src` and
  `semver.tokens` object-identical to the original for `test/internal/re.js`.
- `constants.js`, `lrucache.js`, `parse-options.js`, `debug.js` — pure
  JavaScript data and utilities with no semver semantics. `parse-options.js` in
  particular must return *the caller's own object*, and `debug.js` writes to
  stderr from a child process, neither of which survives a round trip.

`internal/identifiers.js` **is** bridged: `compareIdentifiers` and
`rcompareIdentifiers` call Rust.

`bin/semver.js` execs the Rust CLI with the same argv and propagates its exit
status, so `test/bin/semver.js` compares Rust's output against the original's
recorded snapshots in `tap-snapshots/`.

## Deviations from the original package

- **`test/` is copied, not symlinked.** Node resolves a module's realpath before
  resolving its `require`s, so a symlinked test file looks for
  `../../functions/compare.js` next to `tests/original/` instead of next to the
  adapter. `--preserve-symlinks-main` fixes that for the test file itself but
  not for the sub-processes that `test/internal/debug.js` and
  `test/bin/semver.js` spawn. `scripts/sync-tests.js` runs as `pretest` and
  mirrors `tests/original/` verbatim; the originals are never modified and the
  copy is gitignored.
- **Coverage checking is off.** The original enforces 100% coverage of its own
  JavaScript. Most of that JavaScript is now Rust, so the check is meaningless
  here; `test/map.js` (which asserts that every source file has a matching test
  file) still runs and still passes, which is why `bridge/` sits outside the
  `files` list in `package.json`.
- `sort` and `rsort` use JavaScript's `Array#sort` over a Rust `compareBuild`,
  matching the original exactly including its in-place mutation. Rust's own
  `sort` is covered by `scripts/diff_fuzz.sh` instead.
- `simplifyRange` and `max`/`minSatisfying` hand back the caller's own argument
  or array element when the original would have, but do not sort the caller's
  array in place the way the original's `simplify` does.
