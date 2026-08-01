//! Port of `internal/constants.js`.

/// Note: this is the semver.org version of the spec that it implements,
/// not necessarily the package version of this code.
pub const SEMVER_SPEC_VERSION: &str = "2.0.0";

pub const MAX_LENGTH: usize = 256;

/// `Number.MAX_SAFE_INTEGER`
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Max safe segment length for coercion.
pub const MAX_SAFE_COMPONENT_LENGTH: usize = 16;

/// Max safe length for a build identifier. The max length minus 6 characters
/// for the shortest version with a build `0.0.0+BUILD`.
pub const MAX_SAFE_BUILD_LENGTH: usize = MAX_LENGTH - 6;

pub const RELEASE_TYPES: [&str; 7] = [
    "major",
    "premajor",
    "minor",
    "preminor",
    "patch",
    "prepatch",
    "prerelease",
];

pub const FLAG_INCLUDE_PRERELEASE: u8 = 0b001;
pub const FLAG_LOOSE: u8 = 0b010;
