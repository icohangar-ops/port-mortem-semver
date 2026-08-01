'use strict'

const rpc = require('../bridge/rpc')

const compareIdentifiers = (a, b) => rpc.call('compareIdentifiers', a, b)

const rcompareIdentifiers = (a, b) => rpc.call('rcompareIdentifiers', a, b)

module.exports = {
  compareIdentifiers,
  rcompareIdentifiers,
}
