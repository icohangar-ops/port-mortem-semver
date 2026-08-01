'use strict'

const { MAX_LENGTH } = require('../internal/constants')
const parseOptions = require('../internal/parse-options')
const rpc = require('../bridge/rpc')
const { fullVersion } = require('../bridge/args')

// The property assignment order below mirrors the original constructor so that
// `JSON.stringify(semver)` produces the same key order.
const assign = (self, parsed) => {
  self.raw = parsed.raw
  self.major = parsed.major
  self.minor = parsed.minor
  self.patch = parsed.patch
  self.prerelease = parsed.prerelease
  self.build = parsed.build
  self.version = parsed.version
}

class SemVer {
  constructor (version, options) {
    options = parseOptions(options)

    if (version instanceof SemVer) {
      if (version.loose === !!options.loose &&
        version.includePrerelease === !!options.includePrerelease) {
        return version
      } else {
        version = version.version
      }
    } else if (typeof version !== 'string') {
      throw new TypeError(`Invalid version. Must be a string. Got type "${typeof version}".`)
    }

    // Checked here rather than in Rust so that megabyte-long inputs never
    // cross the bridge.
    if (version.length > MAX_LENGTH) {
      throw new TypeError(
        `version is longer than ${MAX_LENGTH} characters`
      )
    }

    this.options = options
    this.loose = !!options.loose
    this.includePrerelease = !!options.includePrerelease

    assign(this, rpc.call('semverParse', version, options))
  }

  format () {
    this.version = `${this.major}.${this.minor}.${this.patch}`
    if (this.prerelease.length) {
      this.version += `-${this.prerelease.join('.')}`
    }
    return this.version
  }

  toString () {
    return this.version
  }

  compare (other) {
    if (!(other instanceof SemVer)) {
      if (typeof other === 'string' && other === this.version) {
        return 0
      }
      other = new SemVer(other, this.options)
    }

    if (other.version === this.version) {
      return 0
    }

    return rpc.call('semverCompare', this.version, other.version, this.options)
  }

  compareMain (other) {
    if (!(other instanceof SemVer)) {
      other = new SemVer(other, this.options)
    }

    return rpc.call('semverCompareMain', this.version, other.version, this.options)
  }

  comparePre (other) {
    if (!(other instanceof SemVer)) {
      other = new SemVer(other, this.options)
    }

    return rpc.call('semverComparePre', this.version, other.version, this.options)
  }

  compareBuild (other) {
    if (!(other instanceof SemVer)) {
      other = new SemVer(other, this.options)
    }

    return rpc.call(
      'semverCompareBuild',
      fullVersion(this),
      fullVersion(other),
      this.options
    )
  }

  // preminor will bump the version up to the next minor. Then it will bump the
  // prerelease. All the logic lives in Rust; the instance is only mutated once
  // the call has succeeded, so a throwing increment leaves it untouched.
  inc (release, identifier, identifierBase) {
    assign(this, rpc.call(
      'semverInc',
      fullVersion(this),
      release,
      identifier === undefined ? null : identifier,
      identifierBase === undefined ? null : identifierBase,
      this.options
    ))
    this.format()
    this.raw = this.version + (this.build.length ? `+${this.build.join('.')}` : '')
    return this
  }
}

module.exports = SemVer
