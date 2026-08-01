'use strict'

const rpc = require('../bridge/rpc')
const { buildVersionArg } = require('../bridge/args')

const compareBuild = (a, b, loose) =>
  rpc.call('compareBuild', buildVersionArg(a), buildVersionArg(b), loose)

module.exports = compareBuild
