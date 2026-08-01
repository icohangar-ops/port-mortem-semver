'use strict'

const SemVer = require('../classes/semver')
const parseOptions = require('../internal/parse-options')
const rpc = require('../bridge/rpc')
const { semverFromState } = require('../bridge/hydrate')

const coerce = (version, options) => {
  if (version instanceof SemVer) {
    return version
  }

  if (typeof version === 'number') {
    version = String(version)
  }

  if (typeof version !== 'string') {
    return null
  }

  const state = rpc.call('coerce', version, options)
  return state === null ? null : semverFromState(state, parseOptions(options))
}

module.exports = coerce
