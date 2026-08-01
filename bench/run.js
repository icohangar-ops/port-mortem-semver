'use strict'

// CLI-level benchmark: original npm/node-semver vs the Rust port.
//
// Two things are measured, because they fail in opposite directions:
//
//   startup    - one process per invocation, which is how the `semver` CLI is
//                actually used from shell scripts and package managers. This
//                is dominated by runtime boot, not by semver itself.
//   throughput - a single process doing a large batch of comparisons or range
//                checks, which isolates the library.
//
// Results are written to bench/results.json. See bench/methodology.md.

const { spawnSync } = require('child_process')
const fs = require('fs')
const path = require('path')

const ROOT = path.resolve(__dirname, '..')
const NODE_CLI = path.join(ROOT, 'vendor', 'node-semver', 'bin', 'semver.js')
const RUST_CLI = process.env.SEMVER_CLI_BIN || path.join(ROOT, 'target', 'release', 'semver')

const STARTUP_RUNS = Number(process.env.STARTUP_RUNS || 30)
const THROUGHPUT_N = Number(process.env.THROUGHPUT_N || 20000)

const ms = (ns) => Number(ns) / 1e6

const stats = (samples) => {
  const sorted = [...samples].sort((a, b) => a - b)
  const sum = sorted.reduce((a, b) => a + b, 0)
  return {
    runs: sorted.length,
    meanMs: +(sum / sorted.length).toFixed(3),
    medianMs: +sorted[Math.floor(sorted.length / 2)].toFixed(3),
    minMs: +sorted[0].toFixed(3),
    maxMs: +sorted[sorted.length - 1].toFixed(3),
  }
}

const timeOnce = (cmd, args) => {
  const t0 = process.hrtime.bigint()
  const res = spawnSync(cmd, args, { encoding: 'utf8' })
  const t1 = process.hrtime.bigint()
  if (res.error) {
    throw res.error
  }
  return { ms: ms(t1 - t0), stdout: res.stdout, status: res.status }
}

const measure = (label, cmd, args, runs) => {
  // one untimed warm-up so neither side pays for a cold page cache
  timeOnce(cmd, args)
  const samples = []
  for (let i = 0; i < runs; i++) {
    samples.push(timeOnce(cmd, args).ms)
  }
  const s = stats(samples)
  console.log(`  ${label.padEnd(28)} median ${s.medianMs.toFixed(2)}ms  mean ${s.meanMs.toFixed(2)}ms`)
  return s
}

// --- startup ----------------------------------------------------------------

console.log(`startup (${STARTUP_RUNS} runs each)`)

const startup = {}
for (const [name, args] of [
  ['version-check', ['1.2.3']],
  ['compare', ['1.2.3', '2.0.0', '-rv']],
  ['range-filter', ['1.2.3', '2.3.4', '3.0.0', '-r', '^2.0.0']],
]) {
  startup[name] = {
    node: measure(`node   ${name}`, process.execPath, [NODE_CLI, ...args], STARTUP_RUNS),
    rust: measure(`rust   ${name}`, RUST_CLI, args, STARTUP_RUNS),
  }
  startup[name].speedup = +(startup[name].node.medianMs / startup[name].rust.medianMs).toFixed(2)
  console.log(`  -> ${startup[name].speedup}x faster\n`)
}

// --- throughput -------------------------------------------------------------
//
// The CLI sorts every version it is handed and filters by range, so passing N
// versions in one invocation exercises the library N times inside a single
// process. Each scenario is also run with a single version and that time is
// subtracted, which removes runtime boot and one-off setup (regex compilation
// on the Rust side, module loading on the Node side) from both sides.

console.log(`throughput (${THROUGHPUT_N} versions per invocation)`)

const versions = []
for (let i = 0; i < THROUGHPUT_N; i++) {
  const major = i % 7
  const minor = (i * 13) % 23
  const patch = (i * 31) % 47
  const pre = i % 5 === 0 ? `-beta.${i % 11}` : ''
  versions.push(`${major}.${minor}.${patch}${pre}`)
}

const throughput = {}
for (const [name, extra] of [
  ['sort', []],
  ['satisfies', ['-r', '^2.0.0']],
]) {
  const args = [...versions, ...extra]
  const oneArg = [versions[0], ...extra]

  const nodeBaseline = measure(`node   ${name} baseline`, process.execPath,
    [NODE_CLI, ...oneArg], STARTUP_RUNS).medianMs
  const rustBaseline = measure(`rust   ${name} baseline`, RUST_CLI, oneArg,
    STARTUP_RUNS).medianMs

  const nodeRun = timeOnce(process.execPath, [NODE_CLI, ...args])
  const rustRun = timeOnce(RUST_CLI, args)

  const nodeWork = Math.max(nodeRun.ms - nodeBaseline, 0.001)
  const rustWork = Math.max(rustRun.ms - rustBaseline, 0.001)

  throughput[name] = {
    versions: THROUGHPUT_N,
    node: {
      totalMs: +nodeRun.ms.toFixed(3),
      baselineMs: +nodeBaseline.toFixed(3),
      workMs: +nodeWork.toFixed(3),
      versionsPerSecond: Math.round(THROUGHPUT_N / (nodeWork / 1000)),
    },
    rust: {
      totalMs: +rustRun.ms.toFixed(3),
      baselineMs: +rustBaseline.toFixed(3),
      workMs: +rustWork.toFixed(3),
      versionsPerSecond: Math.round(THROUGHPUT_N / (rustWork / 1000)),
    },
    speedup: +(nodeWork / rustWork).toFixed(2),
    sameOutput: nodeRun.stdout === rustRun.stdout,
  }

  console.log(`  ${name}: node ${nodeWork.toFixed(1)}ms of work, rust ${rustWork.toFixed(1)}ms ` +
    `-> ${throughput[name].speedup}x (identical output: ${throughput[name].sameOutput})\n`)
}

// --- report -----------------------------------------------------------------

const results = {
  generatedAt: new Date().toISOString(),
  platform: `${process.platform} ${process.arch}`,
  node: process.version,
  nodeCli: path.relative(ROOT, NODE_CLI),
  rustCli: path.relative(ROOT, RUST_CLI),
  startup,
  throughput,
}

const out = path.join(__dirname, 'results.json')
fs.writeFileSync(out, JSON.stringify(results, null, 2) + '\n')
console.log(`\nwrote ${path.relative(ROOT, out)}`)

const mismatched = Object.entries(throughput).filter(([, v]) => !v.sameOutput)
if (mismatched.length) {
  console.error(`output mismatch in: ${mismatched.map(([k]) => k).join(', ')}`)
  process.exit(1)
}
