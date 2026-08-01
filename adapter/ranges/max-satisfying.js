'use strict'

const rpc = require('../bridge/rpc')
const { versionArg, rangeArg } = require('../bridge/args')

const maxSatisfying = (versions, range, options) => {
  const strings = versions.map(versionArg)
  const max = rpc.call('maxSatisfying', strings, rangeArg(range), options)
  if (max === null) {
    return null
  }
  // Hand back the caller's own element, which may be a SemVer instance.
  return versions[strings.indexOf(max)]
}

module.exports = maxSatisfying
