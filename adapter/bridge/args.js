'use strict'

// Helpers for turning JavaScript call arguments into the plain strings the
// Rust RPC understands.
//
// Type errors are raised here rather than across the bridge: `typeof` is a
// JavaScript-level concern, and JSON cannot tell `undefined` from `null`, so
// the original's exact TypeError text is easier to reproduce in JS.

// The string that round-trips a SemVer's full state, build metadata included.
const fullVersion = (sv) =>
  sv.build && sv.build.length ? `${sv.version}+${sv.build.join('.')}` : sv.version

const isSemVer = (v) =>
  v !== null && typeof v === 'object' && typeof v.version === 'string' &&
  Array.isArray(v.prerelease) && Array.isArray(v.build)

// A version argument as the original's `new SemVer(version, options)` would
// have resolved it, minus build metadata (which comparisons other than
// compareBuild ignore).
const versionArg = (v) => {
  if (isSemVer(v)) {
    return v.version
  }
  if (typeof v !== 'string') {
    throw new TypeError(`Invalid version. Must be a string. Got type "${typeof v}".`)
  }
  return v
}

// Same, but preserving build metadata for compareBuild and friends.
const buildVersionArg = (v) => (isSemVer(v) ? fullVersion(v) : versionArg(v))

// A range argument: Range instances carry their (whitespace-collapsed) source
// in `raw`, which reparses to an identical set.
const rangeArg = (r) => {
  if (r !== null && typeof r === 'object' && typeof r.raw === 'string' && Array.isArray(r.set)) {
    return r.raw
  }
  return r
}

module.exports = { fullVersion, isSemVer, versionArg, buildVersionArg, rangeArg }
