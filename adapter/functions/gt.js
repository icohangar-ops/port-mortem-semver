'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const gt = (a, b, loose) => rpc.call('gt', versionArg(a), versionArg(b), loose)

module.exports = gt
