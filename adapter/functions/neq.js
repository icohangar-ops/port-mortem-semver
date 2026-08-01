'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const neq = (a, b, loose) => rpc.call('neq', versionArg(a), versionArg(b), loose)

module.exports = neq
