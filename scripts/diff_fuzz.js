'use strict'

// Differential fuzzer: generate random versions, ranges and option sets, run
// the whole op matrix through both the original npm/node-semver (vendored, JS)
// and the Rust port (via semver-rpc), and report any divergence in result or
// error message.
//
// Usage: node scripts/diff_fuzz.js [seed] [durationSeconds]

const path = require('path')

const ROOT = path.resolve(__dirname, '..')
const VENDOR = path.join(ROOT, 'vendor', 'node-semver')

const semver = require(VENDOR)
const Comparator = require(path.join(VENDOR, 'classes', 'comparator'))
const Range = require(path.join(VENDOR, 'classes', 'range'))
const rpc = require(path.join(ROOT, 'adapter', 'bridge', 'rpc'))

let seed = Number(process.argv[2] || 1) >>> 0
const DURATION_MS = Number(process.argv[3] || 60) * 1000

const rnd = () => {
  seed = (seed * 1664525 + 1013904223) >>> 0
  return seed / 4294967296
}
const pick = (a) => a[Math.floor(rnd() * a.length)]
const int = (n) => Math.floor(rnd() * n)

// --- generators -------------------------------------------------------------

const NUMS = ['0', '1', '2', '3', '9', '10', '99', '0', '1', '00', '007', 'x', 'X', '*', '']
const PRE = ['', '-alpha', '-alpha.1', '-0', '-1', '-rc.0', '-beta', '-pre.2.3', '-0.0.0',
  '-a.b.c', '-1.2.3', '-alpha.beta.0', '-9007199254740991', '-90071992547409910']
const BUILD = ['', '+build', '+b.1', '+exp.sha.5114f85', '+0']
const PREFIX = ['', 'v', '=', '=v', ' ', 'v ']
const WEIRD = ['', 'x', 'blerg', '1.2', '1', 'a.b.c', '1.2.3.4', 'Infinity.NaN.0', '  ',
  '1.2.3 ', ' 1.2.3', '1.2.3-', '1.2.3+', '\t1.2.3', '1.2.3\n']

const genVersion = () => {
  if (rnd() < 0.08) {
    return pick(WEIRD)
  }
  const n = NUMS.slice(0, 8)
  return `${pick(PREFIX)}${pick(n)}.${pick(n)}.${pick(n)}${pick(PRE)}${pick(BUILD)}`
}

const OPS = ['', '=', '>', '<', '>=', '<=', '~', '^', '~>']

const genSimpleRange = () => {
  if (rnd() < 0.1) {
    return pick(['*', '', 'x', 'X', '*.*', '>=*', '<0.0.0-0', '>=0.0.0', '>=0.0.0-0'])
  }
  let s = pick(OPS)
  if (rnd() < 0.15) {
    s += ' '
  }
  s += pick(NUMS)
  if (rnd() < 0.85) {
    s += '.' + pick(NUMS)
  }
  if (rnd() < 0.75) {
    s += '.' + pick(NUMS)
  }
  if (rnd() < 0.3) {
    s += pick(PRE)
  }
  if (rnd() < 0.15) {
    s += pick(BUILD)
  }
  return s
}

const genRange = () => {
  if (rnd() < 0.12) {
    const bare = () => genSimpleRange().replace(/^[\^~<>=]+/, '')
    return `${bare()} - ${bare()}`
  }
  const parts = []
  for (let i = 1 + int(3); i > 0; i--) {
    const conj = []
    for (let j = 1 + int(2); j > 0; j--) {
      conj.push(genSimpleRange())
    }
    parts.push(conj.join(' '))
  }
  return parts.join(' || ')
}

const OPTS = [null, true, false, {}, { loose: true }, { includePrerelease: true },
  { loose: true, includePrerelease: true }, { rtl: true }, { includePrerelease: true, rtl: true }]

const RELEASES = ['major', 'minor', 'patch', 'premajor', 'preminor', 'prepatch',
  'prerelease', 'release', 'pre', 'bogus']
const TRUNCATIONS = ['major', 'minor', 'patch', 'premajor', 'prerelease', 'bogus']
const CMP_OPS = ['', '=', '==', '!=', '>', '>=', '<', '<=', '===', '!==', '=~']
const IDS = [null, 'alpha', 'beta', 'rc', '', 'a.b', '0', 'bad!']
const BASES = [null, false, '0', '1', 0, 1]
const VERSION_POOL = ['0.0.1', '1.0.0', '1.2.3', '1.5.0', '2.0.0', '2.1.0']

// One round of calls sharing the same random inputs.
const genCalls = () => {
  const v1 = genVersion()
  const v2 = genVersion()
  const rg = genRange()
  const rg2 = genRange()
  const o = pick(OPTS)

  return [
    ['parse', [v1, o]],
    ['valid', [v1, o]],
    ['clean', [v1, o]],
    ['compare', [v1, v2, o]],
    ['compareBuild', [v1, v2, o]],
    ['compareLoose', [v1, v2]],
    ['rcompare', [v1, v2, o]],
    ['gt', [v1, v2, o]],
    ['lt', [v1, v2, o]],
    ['eq', [v1, v2, o]],
    ['neq', [v1, v2, o]],
    ['gte', [v1, v2, o]],
    ['lte', [v1, v2, o]],
    ['cmp', [v1, pick(CMP_OPS), v2, o]],
    ['diff', [v1, v2]],
    ['major', [v1, o]],
    ['minor', [v1, o]],
    ['patch', [v1, o]],
    ['prerelease', [v1, o]],
    ['coerce', [v1, o]],
    ['truncate', [v1, pick(TRUNCATIONS), o]],
    ['inc', [v1, pick(RELEASES), o, pick(IDS), pick(BASES)]],
    ['satisfies', [v1, rg, o]],
    ['validRange', [rg, o]],
    ['rangeFormat', [rg, o]],
    ['rangeTest', [rg, v1, o]],
    ['toComparators', [rg, o]],
    ['minVersion', [rg, o]],
    ['gtr', [v1, rg, o]],
    ['ltr', [v1, rg, o]],
    ['outside', [v1, rg, pick(['>', '<', '!']), o]],
    ['intersects', [rg, rg2, o]],
    ['subset', [rg, rg2, o]],
    ['maxSatisfying', [[v1, v2, '1.2.3', '2.0.0'], rg, o]],
    ['minSatisfying', [[v1, v2, '1.2.3', '2.0.0'], rg, o]],
    ['simplifyRange', [VERSION_POOL, rg, o]],
    ['comparatorParse', [genSimpleRange(), o]],
    ['comparatorIntersects', [genSimpleRange(), genSimpleRange(), o]],
    ['sort', [[v1, v2, '1.2.3', '0.0.1'], o]],
    ['rsort', [[v1, v2, '1.2.3', '0.0.1'], o]],
  ]
}

// --- reference (vendored JavaScript) ----------------------------------------

const plain = (sv) => ({
  raw: sv.raw,
  major: sv.major,
  minor: sv.minor,
  patch: sv.patch,
  prerelease: sv.prerelease,
  build: sv.build,
  version: sv.version,
})

const orNull = (v) => (v === null || v === undefined ? null : v)

const jsCall = (op, args) => {
  // JSON turns `null` into an explicit argument; the JS API expects the
  // argument to simply be absent.
  const a = args.map((x) => (x === null ? undefined : x))
  switch (op) {
    case 'parse':
    case 'coerce':
    case 'minVersion': {
      const r = semver[op](a[0], a[1])
      return r === null || r === undefined ? null : plain(r)
    }
    case 'cmp':
    case 'outside':
      return semver[op](a[0], a[1], a[2], a[3])
    case 'truncate':
      return orNull(semver.truncate(a[0], a[1], a[2]))
    case 'inc':
      return orNull(semver.inc(a[0], a[1], a[2], a[3], a[4]))
    case 'rangeFormat':
      return new Range(a[0], a[1]).range
    case 'rangeTest':
      return new Range(a[0], a[2]).test(a[1])
    case 'comparatorParse': {
      const c = new Comparator(a[0], a[1])
      return { operator: c.operator, value: c.value }
    }
    case 'comparatorIntersects':
      return new Comparator(a[0], a[2]).intersects(new Comparator(a[1], a[2]), a[2])
    case 'simplifyRange':
      return String(semver.simplifyRange(a[0].slice(), a[1], a[2]))
    case 'sort':
    case 'rsort':
      return semver[op](a[0].slice(), a[1])
    case 'compareLoose':
      return semver.compareLoose(a[0], a[1])
    default:
      return orNull(semver[op](a[0], a[1], a[2], a[3]))
  }
}

// --- comparison -------------------------------------------------------------

// Only the fields both sides model are compared; the Rust structs do not carry
// JS-only bookkeeping such as `options`.
const canon = (v) => {
  if (v && typeof v === 'object' && !Array.isArray(v)) {
    const o = {}
    for (const k of ['raw', 'major', 'minor', 'patch', 'prerelease', 'build', 'version',
      'operator', 'value']) {
      if (k in v) {
        o[k] = v[k]
      }
    }
    return JSON.stringify(o)
  }
  return JSON.stringify(v)
}

const run = () => {
  const started = Date.now()
  const byOp = {}
  const divergences = []
  let total = 0

  while (Date.now() - started < DURATION_MS) {
    for (const [op, args] of genCalls()) {
      total++

      let js
      try {
        js = { ok: true, result: jsCall(op, args) }
      } catch (e) {
        js = { ok: false, error: e.message }
      }

      const rs = rpc.send(op, args)

      const same = js.ok === rs.ok &&
        (js.ok ? canon(js.result) === canon(rs.result) : js.error === rs.error)

      if (!same) {
        byOp[op] = (byOp[op] || 0) + 1
        if (divergences.length < 50) {
          divergences.push({ op, args, js, rust: rs })
        }
      }
    }
  }

  return { total, byOp, divergences, elapsed: Date.now() - started }
}

const { total, byOp, divergences, elapsed } = run()
const count = Object.values(byOp).reduce((a, b) => a + b, 0)

const lines = []
lines.push(`seed: ${process.argv[2] || 1}`)
lines.push(`duration: ${(elapsed / 1000).toFixed(1)}s`)
lines.push(`calls: ${total}`)
lines.push(`divergences: ${count}`)
for (const d of divergences) {
  lines.push('')
  lines.push(`DIVERGENCE [${d.op}] args=${JSON.stringify(d.args)}`)
  lines.push(`  node-semver: ${JSON.stringify(d.js)}`)
  lines.push(`  rust       : ${JSON.stringify(d.rust)}`)
}
if (count) {
  lines.push('')
  lines.push(`by op: ${JSON.stringify(byOp)}`)
}

process.stdout.write(lines.join('\n') + '\n')
process.exit(count ? 1 : 0)
