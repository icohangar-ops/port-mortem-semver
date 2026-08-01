'use strict'

const rpc = require('../bridge/rpc')
const { versionArg, rangeArg } = require('../bridge/args')

const minSatisfying = (versions, range, options) => {
  const strings = versions.map(versionArg)
  const min = rpc.call('minSatisfying', strings, rangeArg(range), options)
  if (min === null) {
    return null
  }
  // Hand back the caller's own element, which may be a SemVer instance.
  return versions[strings.indexOf(min)]
}

module.exports = minSatisfying
