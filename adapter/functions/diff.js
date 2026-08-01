'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const diff = (version1, version2) => rpc.call('diff', versionArg(version1), versionArg(version2))

module.exports = diff
