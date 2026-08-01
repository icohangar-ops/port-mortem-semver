'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const minor = (a, loose) => rpc.call('minor', versionArg(a), loose)

module.exports = minor
