'use strict'

const rpc = require('../bridge/rpc')
const SemVer = require('../classes/semver')

// The `(version, release, identifier, identifierBase)` overload, error
// swallowing and increment itself all live in Rust.
const inc = (version, release, options, identifier, identifierBase) =>
  rpc.call(
    'inc',
    version instanceof SemVer ? version.version : version,
    release,
    options,
    identifier,
    identifierBase
  )

module.exports = inc
