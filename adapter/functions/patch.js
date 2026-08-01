'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const patch = (a, loose) => rpc.call('patch', versionArg(a), loose)

module.exports = patch
