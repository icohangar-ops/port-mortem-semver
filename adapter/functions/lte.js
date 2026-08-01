'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const lte = (a, b, loose) => rpc.call('lte', versionArg(a), versionArg(b), loose)

module.exports = lte
