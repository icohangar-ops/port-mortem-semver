//! Error type covering every `throw` site in node-semver.
//!
//! The `Display` text is byte-for-byte identical to the JavaScript messages so
//! that parity harnesses can compare them directly.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SemverError {
    /// `TypeError: Invalid Version: ${version}`
    #[error("Invalid Version: {0}")]
    InvalidVersion(String),

    /// `TypeError: version is longer than ${MAX_LENGTH} characters`
    #[error("version is longer than {0} characters")]
    VersionTooLong(usize),

    /// `TypeError: Invalid version. Must be a string. Got type "${typeof version}".`
    #[error("Invalid version. Must be a string. Got type \"{0}\".")]
    InvalidVersionType(String),

    #[error("Invalid major version")]
    InvalidMajor,

    #[error("Invalid minor version")]
    InvalidMinor,

    #[error("Invalid patch version")]
    InvalidPatch,

    /// `TypeError: Invalid comparator: ${comp}`
    #[error("Invalid comparator: {0}")]
    InvalidComparator(String),

    /// `TypeError: Invalid SemVer Range: ${range}`
    #[error("Invalid SemVer Range: {0}")]
    InvalidRange(String),

    /// `TypeError: Invalid operator: ${op}`
    #[error("Invalid operator: {0}")]
    InvalidOperator(String),

    /// `Error: invalid increment argument: ${release}`
    #[error("invalid increment argument: {0}")]
    InvalidIncrement(String),

    /// `Error: invalid identifier: ${identifier}`
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),

    /// `Error: version ${raw} is not a prerelease`
    #[error("version {0} is not a prerelease")]
    NotAPrerelease(String),

    /// `TypeError: Must provide a hilo val of "<" or ">"`
    #[error("Must provide a hilo val of \"<\" or \">\"")]
    InvalidHilo,

    /// `Error: Unexpected operation: ${operator}`
    #[error("Unexpected operation: {0}")]
    UnexpectedOperation(String),

    /// Anything else (mirrors ad-hoc `throw new TypeError(msg)` sites).
    #[error("{0}")]
    Other(String),
}

impl SemverError {
    /// The `name` of the JS error that would have been thrown, useful for
    /// differential testing against Node.
    pub fn js_error_name(&self) -> &'static str {
        match self {
            SemverError::InvalidIncrement(_)
            | SemverError::InvalidIdentifier(_)
            | SemverError::NotAPrerelease(_)
            | SemverError::UnexpectedOperation(_) => "Error",
            _ => "TypeError",
        }
    }
}

pub type Result<T> = std::result::Result<T, SemverError>;
