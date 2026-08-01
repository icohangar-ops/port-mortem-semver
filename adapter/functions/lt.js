'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const lt = (a, b, loose) => rpc.call('lt', versionArg(a), versionArg(b), loose)

module.exports = lt
