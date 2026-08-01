'use strict'

const rpc = require('../bridge/rpc')
const { versionArg, rangeArg } = require('../bridge/args')

// A non-string hilo is stringified rather than rejected up front: the original
// parses the version and range first, then falls through its switch to
// `Must provide a hilo val of "<" or ">"`, and Rust does the same.
const outside = (version, range, hilo, options) =>
  rpc.call(
    'outside',
    versionArg(version),
    rangeArg(range),
    typeof hilo === 'string' ? hilo : String(hilo),
    options
  )

module.exports = outside
