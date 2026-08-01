'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const compare = (a, b, loose) => rpc.call('compare', versionArg(a), versionArg(b), loose)

module.exports = compare
