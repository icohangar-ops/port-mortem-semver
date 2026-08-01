'use strict'

const rpc = require('../bridge/rpc')
const { versionArg } = require('../bridge/args')

const major = (a, loose) => rpc.call('major', versionArg(a), loose)

module.exports = major
