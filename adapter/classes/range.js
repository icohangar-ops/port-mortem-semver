'use strict'

const SPACE_CHARACTERS = /\s+/g

// hoisted class for cyclic dependency
class Range {
  constructor (range, options) {
    options = parseOptions(options)

    if (range instanceof Range) {
      if (
        range.loose === !!options.loose &&
        range.includePrerelease === !!options.includePrerelease
      ) {
        return range
      } else {
        return new Range(range.raw, options)
      }
    }

    if (range instanceof Comparator) {
      // just put it in the set and return
      this.raw = range.value
      this.set = [[range]]
      this.formatted = undefined
      return this
    }

    this.options = options
    this.loose = !!options.loose
    this.includePrerelease = !!options.includePrerelease

    // First reduce all whitespace as much as possible so we do not have to
    // ship megabytes of padding across the bridge.
    this.raw = range.trim().replace(SPACE_CHARACTERS, ' ')

    // First, split on ||
    this.set = this.raw
      .split('||')
      // map the range to a 2d array of comparators
      .map(r => this.parseRange(r.trim()))
      // throw out any comparator lists that are empty
      // this generally means that it was not a valid range, which is allowed
      // in loose mode, but will still throw if the WHOLE range is invalid.
      .filter(c => c.length)

    if (!this.set.length) {
      throw new TypeError(`Invalid SemVer Range: ${this.raw}`)
    }

    // if we have any that are not the null set, throw out null sets.
    if (this.set.length > 1) {
      // keep the first one, in case they're all null sets
      const first = this.set[0]
      this.set = this.set.filter(c => !isNullSet(c[0]))
      if (this.set.length === 0) {
        this.set = [first]
      } else if (this.set.length > 1) {
        // if we have any that are *, then the range is just *
        for (const c of this.set) {
          if (c.length === 1 && isAny(c[0])) {
            this.set = [c]
            break
          }
        }
      }
    }

    this.formatted = undefined
  }

  get range () {
    if (this.formatted === undefined) {
      this.formatted = ''
      for (let i = 0; i < this.set.length; i++) {
        if (i > 0) {
          this.formatted += '||'
        }
        const comps = this.set[i]
        for (let k = 0; k < comps.length; k++) {
          if (k > 0) {
            this.formatted += ' '
          }
          this.formatted += comps[k].toString().trim()
        }
      }
    }
    return this.formatted
  }

  format () {
    return this.range
  }

  toString () {
    return this.range
  }

  // All of the desugaring (hyphen ranges, ~, ^, x-ranges, >=0.0.0, loose
  // filtering, dedupe) happens in Rust. This memoizes the resulting comparator
  // array exactly as the original does, so repeated ranges share one array.
  parseRange (range) {
    // strip build metadata so it can't bleed into the version, and so the memo
    // key matches the one Rust computes
    range = range.replace(BUILDSTRIPRE, '')

    const memoOpts =
      (this.options.includePrerelease && FLAG_INCLUDE_PRERELEASE) |
      (this.options.loose && FLAG_LOOSE)
    const memoKey = memoOpts + ':' + range
    const cached = cache.get(memoKey)
    if (cached) {
      return cached
    }

    const result = rpc.call('parseRangeSet', range, this.options)
      .map(state => comparatorFromState(state, this.options))
    cache.set(memoKey, result)
    return result
  }

  intersects (range, options) {
    if (!(range instanceof Range)) {
      throw new TypeError('a Range is required')
    }

    return rpc.call(
      'rangeIntersects',
      this.raw,
      range.raw,
      options === undefined ? null : options,
      this.options,
      range.options
    )
  }

  // if ANY of the sets match ALL of its comparators, then pass
  test (version) {
    if (!version) {
      return false
    }

    if (typeof version !== 'string') {
      if (!isSemVer(version)) {
        return false
      }
      version = fullVersion(version)
    }

    return rpc.tryCall(false, 'rangeTest', this.raw, version, this.options)
  }
}

module.exports = Range

const parseOptions = require('../internal/parse-options')
const rpc = require('../bridge/rpc')
const { comparatorFromState } = require('../bridge/hydrate')
const { fullVersion, isSemVer } = require('../bridge/args')
const LRU = require('../internal/lrucache')
const cache = new LRU()

const {
  FLAG_INCLUDE_PRERELEASE,
  FLAG_LOOSE,
} = require('../internal/constants')

const { src, t } = require('../internal/re')
const BUILDSTRIPRE = new RegExp(src[t.BUILD], 'g')

const Comparator = require('./comparator')

const isNullSet = c => c.value === '<0.0.0-0'
const isAny = c => c.value === ''
