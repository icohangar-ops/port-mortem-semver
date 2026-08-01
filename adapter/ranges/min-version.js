'use strict'

const parseOptions = require('../internal/parse-options')
const rpc = require('../bridge/rpc')
const { semverFromState } = require('../bridge/hydrate')
const { rangeArg } = require('../bridge/args')

const minVersion = (range, loose) => {
  const state = rpc.call('minVersion', rangeArg(range), loose)
  return state === null ? null : semverFromState(state, parseOptions(undefined))
}

module.exports = minVersion
