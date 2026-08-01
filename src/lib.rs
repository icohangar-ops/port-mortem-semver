//! A faithful Rust port of [npm/node-semver](https://github.com/npm/node-semver).
//!
//! The module layout mirrors the JavaScript package:
//!
//! | JavaScript | Rust |
//! | --- | --- |
//! | `internal/constants.js` | [`constants`] |
//! | `internal/identifiers.js` | [`identifiers`] |
//! | `internal/parse-options.js` | [`options`] |
//! | `internal/re.js` | [`re`] |
//! | `classes/semver.js` | [`semver`] |
//! | `classes/comparator.js` | [`comparator`] |
//! | `classes/range.js` | [`range`] |
//! | `functions/*.js` | [`functions`] |
//! | `ranges/*.js` | [`ranges_api`] |
//!
//! Semantics follow npm, not Cargo: loose parsing, `includePrerelease`, and the
//! caret/tilde/x-range desugaring all behave exactly as they do in Node.

#![forbid(unsafe_code)]

pub mod comparator;
pub mod constants;
pub mod error;
pub mod functions;
pub mod identifiers;
pub mod options;
pub mod range;
pub mod ranges_api;
pub mod re;
pub mod semver;

// --- classes ---------------------------------------------------------------
pub use comparator::Comparator;
pub use range::{Range, ToRange};
pub use semver::{IdentifierBase, SemVer, ToSemVer};

// --- internals -------------------------------------------------------------
pub use constants::{RELEASE_TYPES, SEMVER_SPEC_VERSION};
pub use error::{Result, SemverError};
pub use identifiers::{
    compare_identifiers, compare_identifiers_str, rcompare_identifiers, rcompare_identifiers_str,
    Identifier,
};
pub use options::Options;

// --- functions/* -----------------------------------------------------------
pub use functions::{
    clean, cmp, cmp_semver, coerce, compare, compare_build, compare_loose, diff, eq, gt, gte, inc,
    lt, lte, major, minor, neq, parse, parse_throw, patch, prerelease, rcompare, rsort, satisfies,
    satisfies_semver, sort, truncate, valid,
};

// --- ranges/* --------------------------------------------------------------
pub use ranges_api::{
    gtr, intersects, ltr, max_satisfying, min_satisfying, min_version, outside, simplify, subset,
    to_comparators, valid_range,
};

/// Alias matching the JS export name `simplifyRange`.
pub use ranges_api::simplify as simplify_range;

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn readme_style_usage() {
        assert_eq!(valid("1.2.3", Options::EMPTY).as_deref(), Some("1.2.3"));
        assert_eq!(valid("a.b.c", Options::EMPTY), None);
        assert_eq!(clean("  =v1.2.3   ", Options::EMPTY).as_deref(), Some("1.2.3"));
        assert!(satisfies("1.2.3", "1.x || >=2.5.0 || 5.0.0 - 7.2.3", Options::EMPTY));
        assert!(gt("1.2.3", "9.8.7", Options::EMPTY).is_ok());
        assert_eq!(
            valid_range(">=1.0.0", Options::EMPTY).as_deref(),
            Some(">=1.0.0")
        );
    }

    #[test]
    fn loose_mode() {
        assert_eq!(valid("=1.2.3", Options::EMPTY), None);
        assert_eq!(valid("=1.2.3", Options::LOOSE).as_deref(), Some("1.2.3"));
        assert!(satisfies("1.2.3", "=1.2.3", Options::EMPTY));
    }

    #[test]
    fn include_prerelease() {
        let opts = Options::EMPTY.include_prerelease(true);
        assert!(!satisfies("1.2.4-beta.1", "^1.2.3", Options::EMPTY));
        assert!(satisfies("1.2.4-beta.1", "^1.2.3", opts));
    }

    #[test]
    fn spec_version() {
        assert_eq!(SEMVER_SPEC_VERSION, "2.0.0");
        assert_eq!(RELEASE_TYPES.len(), 7);
    }
}
