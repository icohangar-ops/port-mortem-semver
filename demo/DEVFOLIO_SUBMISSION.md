# Devfolio submission draft — Port Mortem 2026

Paste these into https://portmortem.devfolio.co/dashboard → **Add Project** → Publish.

## Project Name
node-semver-rs

## Tagline
npm's SemVer engine, rewritten in safe Rust — original suite green, zero divergences, zero unsafe.

## Problem it solves
Cross-language ports are easy to generate and hard to prove. This project ports `npm/node-semver` (JS → Rust, Track F) and proves behavioral equivalence with the original, unmodified test suite, a 60s differential fuzzer, and honest CLI benchmarks — without editing tests or using `unsafe`.

## Challenges you ran into
- Matching npm's exact error text and V8 `Array#sort` throw order, not just comparison results
- Preserving JavaScript float stringification for huge range bounds (`1e+25`) that bigint-correct math would "fix"
- Bridging a synchronous Node API to Rust without N-API/`unsafe` (FIFO-backed JSON-lines RPC)
- Making the Rust regex path as faithful *and* as fast as Node (ASCII `\d`, lazy compile, capped lazy-DFA)

## Technologies used
Rust, regex, clap, Node.js (thin test adapter only), tap, differential fuzzing, Docker

## Links
- GitHub (primary): https://github.com/Cubiczan/port-mortem-semver
- GitHub (mirror): https://github.com/icohangar-ops/port-mortem-semver
- Codeberg: https://codeberg.org/cubiczan/port-mortem-semver
- Demo video: _(paste YouTube URL after upload)_

## Video Demo
Preferred: YouTube unlisted/public link to `demo/port-mortem-semver-demo.mp4`.

## Screenshot captions (upload 2–4 images)
1. Cover: CLI sorting versions / README header
2. `make parity` — 9182/9182 pass
3. `fuzz/log.txt` — 3.4M calls, 0 divergences
4. Bench table — ~23× startup, ~10× satisfies

## One-liner for judges
`make build && make parity` — original hashed suite, untouched, all green against the Rust port.
