'use strict'

const ANY = Symbol('SemVer ANY')

// hoisted class for cyclic dependency
class Comparator {
  static get ANY () {
    return ANY
  }

  constructor (comp, options) {
    options = parseOptions(options)

    if (comp instanceof Comparator) {
      if (comp.loose === !!options.loose) {
        return comp
      } else {
        comp = comp.value
      }
    }

    // Collapsed up front so that pathological whitespace never crosses the
    // bridge; the original normalises identically before parsing.
    comp = comp.trim().split(/\s+/).join(' ')
    this.options = options
    this.loose = !!options.loose
    this.parse(comp)

    if (this.semver === ANY) {
      this.value = ''
    } else {
      this.value = this.operator + this.semver.version
    }
  }

  parse (comp) {
    const parsed = rpc.call('comparatorParse', comp, this.options)
    this.operator = parsed.operator
    this.semver = parsed.semver === 'ANY'
      ? ANY
      : semverFromState(parsed.semver, parseOptions(this.options.loose))
  }

  toString () {
    return this.value
  }

  test (version) {
    if (this.semver === ANY || version === ANY) {
      return true
    }

    if (typeof version === 'string') {
      return rpc.tryCall(false, 'comparatorTest', this.value, version, this.options)
    }

    return cmp(version, this.operator, this.semver, this.options)
  }

  intersects (comp, options) {
    if (!(comp instanceof Comparator)) {
      throw new TypeError('a Comparator is required')
    }

    // `value` is the canonical form of a comparator, so it is all the Rust
    // side needs to rebuild an identical one.
    return rpc.call(
      'comparatorIntersects',
      this.value,
      comp.value,
      options === undefined ? null : options,
      this.options,
      comp.options
    )
  }
}

module.exports = Comparator

const parseOptions = require('../internal/parse-options')
const rpc = require('../bridge/rpc')
const { semverFromState } = require('../bridge/hydrate')
const cmp = require('../functions/cmp')
