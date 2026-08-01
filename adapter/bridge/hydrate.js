'use strict'

const parseOptions = require('../internal/parse-options')

// Rebuild class instances from the state the Rust RPC returned, without paying
// for another round trip to reparse what it already parsed. `Object.create`
// keeps `instanceof` working and the assignment order matches the real
// constructors so `JSON.stringify` output is unchanged.

const semverFromState = (state, options) => {
  const SemVer = require('../classes/semver')
  const sv = Object.create(SemVer.prototype)
  sv.options = options
  sv.loose = !!options.loose
  sv.includePrerelease = !!options.includePrerelease
  sv.raw = state.raw
  sv.major = state.major
  sv.minor = state.minor
  sv.patch = state.patch
  sv.prerelease = state.prerelease
  sv.build = state.build
  sv.version = state.version
  return sv
}

const comparatorFromState = (state, options) => {
  const Comparator = require('../classes/comparator')
  const c = Object.create(Comparator.prototype)
  c.options = options
  c.loose = !!options.loose
  c.operator = state.operator
  c.semver = state.semver === 'ANY'
    ? Comparator.ANY
    : semverFromState(state.semver, parseOptions(options.loose))
  c.value = state.value
  return c
}

module.exports = { semverFromState, comparatorFromState }
