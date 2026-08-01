'use strict'

// given a set of versions and a range, create a "simplified" range
// that includes the same versions that the original range does
// If the original range is shorter than the simplified one, return that.
const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

module.exports = (versions, range, options) => {
  const original = typeof range.raw === 'string' ? range.raw : String(range)
  const simplified = rpc.call(
    'simplifyRange',
    versions.map(versionArg),
    original,
    options
  )
  // Rust returns the original text when it could not do better; the original
  // hands back the caller's own `range` value in that case.
  return simplified === original ? range : simplified
}
