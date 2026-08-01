'use strict'

const rpc = require('../bridge/rpc')
const SemVer = require('../classes/semver')

const truncate = (version, truncation, options) =>
  rpc.call(
    'truncate',
    version instanceof SemVer ? version.version : version,
    truncation,
    options
  )

module.exports = truncate
