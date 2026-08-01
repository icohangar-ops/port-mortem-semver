'use strict'

const rpc = require('../bridge/rpc')
const { rangeArg } = require('../bridge/args')

const subset = (sub, dom, options = {}) => {
  if (sub === dom) {
    return true
  }

  return rpc.call('subset', rangeArg(sub), rangeArg(dom), options)
}

module.exports = subset
