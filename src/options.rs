//! Port of `internal/parse-options.js`.
//!
//! JavaScript accepts `undefined`, a boolean, or an options object. A truthy
//! non-object is treated as `{ loose: true }`. In Rust the options are always a
//! value type, and `Options::from(bool)` reproduces the coercion.

use crate::constants::{FLAG_INCLUDE_PRERELEASE, FLAG_LOOSE};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Options {
    pub loose: bool,
    pub include_prerelease: bool,
    pub rtl: bool,
}

impl Options {
    pub const EMPTY: Options = Options {
        loose: false,
        include_prerelease: false,
        rtl: false,
    };

    pub const LOOSE: Options = Options {
        loose: true,
        include_prerelease: false,
        rtl: false,
    };

    pub const fn new() -> Self {
        Options::EMPTY
    }

    pub const fn loose(mut self, loose: bool) -> Self {
        self.loose = loose;
        self
    }

    pub const fn include_prerelease(mut self, include_prerelease: bool) -> Self {
        self.include_prerelease = include_prerelease;
        self
    }

    pub const fn rtl(mut self, rtl: bool) -> Self {
        self.rtl = rtl;
        self
    }

    /// The memoization key bits used by `Range#parseRange`.
    pub(crate) fn memo_flags(&self) -> u8 {
        (if self.include_prerelease {
            FLAG_INCLUDE_PRERELEASE
        } else {
            0
        }) | (if self.loose { FLAG_LOOSE } else { 0 })
    }

    /// `parseOptions(this.options.loose)` — comparators construct their inner
    /// `SemVer` with only the loose bit forwarded.
    pub(crate) fn only_loose(&self) -> Options {
        Options {
            loose: self.loose,
            include_prerelease: false,
            rtl: false,
        }
    }
}

impl From<bool> for Options {
    fn from(loose: bool) -> Self {
        if loose {
            Options::LOOSE
        } else {
            Options::EMPTY
        }
    }
}

impl From<Option<bool>> for Options {
    fn from(loose: Option<bool>) -> Self {
        Options::from(loose.unwrap_or(false))
    }
}
