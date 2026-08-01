# node-semver → Rust (Port Mortem 2026 · Track F)

Faithful Rust rewrite of [`npm/node-semver`](https://github.com/npm/node-semver) (JS → Rust).
The north star is behavioral equivalence: the **original, unmodified** tap suite passes against the port, differential fuzzing finds zero divergences, and there is **zero `unsafe`**.

| | |
| --- | --- |
| Track | **F** · JavaScript → Rust |
| Source | `npm/node-semver@6e05b7637396ac66522cff8731f07cfe0ef49a29` (v7.8.5) |
| Source LOC | ~2.9k |
| Target | Idiomatic safe Rust, single static binaries |
| Test parity | **9,182 / 9,182** assertions · **51 / 51** files · **0 edits** to originals |
| Diff fuzz | **3,445,400** calls · **60s** · **0** divergences |
| `unsafe` | **0** (`#![forbid(unsafe_code)]`) |

## Why this repo

Track F rewards a Node-only tool shipped as a static binary with CLI parity and original-suite survival.
`node-semver` is:

- Pure, deterministic logic (ideal differential-fuzz surface)
- Small enough to finish in 72h (~2.9k LOC) with a dense, high-quality suite
- Universally understood by judges (npm's own SemVer engine)
- A clear modernization story: eliminate the Node runtime for CLI use

## One-command build

```bash
make build          # cargo build --release → target/release/semver
# or
docker build -t node-semver-rs .
```

```bash
./target/release/semver 1.2.3 2.0.0 1.5.0
# 1.2.3
# 1.5.0
# 2.0.0

./target/release/semver -r '^1' 1.2.3 2.0.0 1.9.9
# 1.2.3
# 1.9.9
```

## Prove it

```bash
make parity         # original suite vs Rust via thin Node adapter
make fuzz           # 60s differential fuzzer → fuzz/log.txt
make bench          # CLI startup + throughput → bench/results.json
```

Original tests are hashed at kickoff in `tests/original.SHA256SUMS` and live unmodified under `tests/original/`.
The adapter under `adapter/` is the thin bridge Port Mortem describes — it does **not** call the original JavaScript implementation for semver decisions.

## Benchmarks (honest numbers)

Same CLI surface, same argv, byte-identical stdout required. Full methodology in [`bench/methodology.md`](bench/methodology.md).

| Scenario | Node median | Rust median | Speedup |
| --- | ---: | ---: | ---: |
| Startup · version check | 46.1 ms | 2.0 ms | **22.7×** |
| Startup · range filter | 48.5 ms | 6.2 ms | **7.8×** |
| Throughput · sort 20k | 469 ms | 208 ms | **2.3×** |
| Throughput · satisfies 20k | 380 ms | 37 ms | **10.3×** |

Startup wins come mostly from dropping V8 boot. Throughput wins are library work after subtracting a one-version baseline.

## Layout

```
src/                 idiomatic Rust port (forbid unsafe)
adapter/             thin Node adapter → semver-rpc (runs original tests)
tests/original/      hashed kickoff suite (unmodified)
vendor/node-semver/  original for differential comparison
fuzz/                differential harness log
bench/               methodology + results.json
DECISIONS.md         architectural divergences + why
.port-mortem.toml    track metadata + pinned source commit
```

## Migration rationale

JavaScript is the wrong runtime for a pure comparison/parsing library invoked from shells and CI thousands of times a day.
Rust gives a single static binary, predictable startup, and memory safety without GC — without changing the SemVer contract npm depends on.

See [`DECISIONS.md`](DECISIONS.md) for every non-trivial divergence from the original.

## License

ISC, matching the original.
