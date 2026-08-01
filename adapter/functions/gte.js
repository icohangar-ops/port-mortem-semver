'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const gte = (a, b, loose) => rpc.call('gte', versionArg(a), versionArg(b), loose)

module.exports = gte
