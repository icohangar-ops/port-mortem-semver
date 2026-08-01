'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const compareLoose = (a, b) => rpc.call('compareLoose', versionArg(a), versionArg(b))

module.exports = compareLoose
