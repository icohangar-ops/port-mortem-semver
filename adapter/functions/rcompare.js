'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const rcompare = (a, b, loose) => rpc.call('rcompare', versionArg(a), versionArg(b), loose)

module.exports = rcompare
