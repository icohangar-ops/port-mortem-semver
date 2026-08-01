'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const eq = (a, b, loose) => rpc.call('eq', versionArg(a), versionArg(b), loose)

module.exports = eq
