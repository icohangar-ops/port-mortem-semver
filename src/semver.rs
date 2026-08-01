//! Port of `classes/semver.js`.

use std::cmp::Ordering;
use std::fmt;

use crate::constants::{MAX_LENGTH, MAX_SAFE_INTEGER};
use crate::error::{Result, SemverError};
use crate::identifiers::{compare_identifiers, compare_identifiers_str, Identifier};
use crate::options::Options;
use crate::re::{safe_re, t};

/// The `identifierBase` argument of `SemVer#inc`.
///
/// JavaScript accepts `undefined`, `false`, `'0'`, `'1'` (or the numbers). The
/// value is used two ways: `Number(identifierBase) ? 1 : 0` picks the starting
/// number, and `identifierBase === false` suppresses the number entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdentifierBase {
    /// `undefined` — behaves like `0` but is not `=== false`.
    #[default]
    Unset,
    Zero,
    One,
    /// Literal `false` — omit the numeric suffix.
    False,
}

impl IdentifierBase {
    /// `Number(identifierBase) ? 1 : 0`
    pub fn base(self) -> u64 {
        match self {
            IdentifierBase::One => 1,
            _ => 0,
        }
    }

    pub fn is_false(self) -> bool {
        matches!(self, IdentifierBase::False)
    }

    /// Mirrors the CLI's `-n <base>` handling.
    pub fn from_cli(value: &str) -> IdentifierBase {
        if value == "false" {
            return IdentifierBase::False;
        }
        if js_number(value).map(|n| n != 0.0).unwrap_or(false) {
            IdentifierBase::One
        } else {
            IdentifierBase::Zero
        }
    }
}

/// A very small `Number(string)` approximation, enough for `isNaN` checks and
/// truthiness of `identifierBase`.
pub(crate) fn js_number(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }
    match trimmed {
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    trimmed.parse::<f64>().ok().filter(|n| !n.is_nan())
}

/// `String(number)` per ECMAScript `Number::toString`.
///
/// Rust's `Display` already produces the shortest round-tripping decimal, but
/// JavaScript switches to exponential notation at 1e21 (so `+'9'.repeat(25) + 1`
/// stringifies as `1e+25`, not as 26 digits). Range desugaring interpolates
/// these values straight into comparator text, so the difference is observable.
pub(crate) fn js_number_to_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if n == 0.0 {
        return "0".to_string();
    }
    if n.abs() >= 1e21 {
        let exp = format!("{n:e}");
        return match exp.split_once('e') {
            Some((mantissa, e)) if !e.starts_with('-') => format!("{mantissa}e+{e}"),
            _ => exp,
        };
    }
    format!("{n}")
}

/// Anything that can be turned into a [`SemVer`], mirroring the JS habit of
/// accepting either a string or a `SemVer` instance.
pub trait ToSemVer {
    fn to_semver(&self, options: Options) -> Result<SemVer>;
}

impl ToSemVer for str {
    fn to_semver(&self, options: Options) -> Result<SemVer> {
        SemVer::new(self, options)
    }
}

impl ToSemVer for String {
    fn to_semver(&self, options: Options) -> Result<SemVer> {
        SemVer::new(self, options)
    }
}

impl ToSemVer for SemVer {
    fn to_semver(&self, options: Options) -> Result<SemVer> {
        // `new SemVer(semverInstance, options)` returns the same instance when
        // the flags agree, otherwise it re-parses `version` (dropping build).
        if self.loose == options.loose && self.include_prerelease == options.include_prerelease {
            Ok(self.clone())
        } else {
            SemVer::new(&self.version, options)
        }
    }
}

impl<T: ToSemVer + ?Sized> ToSemVer for &T {
    fn to_semver(&self, options: Options) -> Result<SemVer> {
        (**self).to_semver(options)
    }
}

#[derive(Debug, Clone)]
pub struct SemVer {
    pub options: Options,
    pub loose: bool,
    pub include_prerelease: bool,
    pub raw: String,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Vec<Identifier>,
    pub build: Vec<String>,
    /// The normalized `major.minor.patch[-prerelease]` string.
    pub version: String,
}

impl SemVer {
    pub fn new(version: &str, options: Options) -> Result<SemVer> {
        // JS measures `version.length` in UTF-16 code units.
        if version.len() > MAX_LENGTH && version.encode_utf16().count() > MAX_LENGTH {
            return Err(SemverError::VersionTooLong(MAX_LENGTH));
        }

        let re = if options.loose {
            safe_re(t::LOOSE)
        } else {
            safe_re(t::FULL)
        };

        let trimmed = version.trim();
        let m = match re.captures(trimmed) {
            Some(m) => m,
            None => return Err(SemverError::InvalidVersion(version.to_string())),
        };

        let major = parse_component(&m[1]).ok_or(SemverError::InvalidMajor)?;
        let minor = parse_component(&m[2]).ok_or(SemverError::InvalidMinor)?;
        let patch = parse_component(&m[3]).ok_or(SemverError::InvalidPatch)?;

        let prerelease = match m.get(4) {
            None => Vec::new(),
            Some(pre) => pre.as_str().split('.').map(Identifier::parse).collect(),
        };

        let build = match m.get(5) {
            None => Vec::new(),
            Some(b) => b.as_str().split('.').map(|s| s.to_string()).collect(),
        };

        let mut sv = SemVer {
            options,
            loose: options.loose,
            include_prerelease: options.include_prerelease,
            raw: version.to_string(),
            major,
            minor,
            patch,
            prerelease,
            build,
            version: String::new(),
        };
        sv.format();
        Ok(sv)
    }

    /// `new SemVer(version)` with default options.
    pub fn parse(version: &str) -> Result<SemVer> {
        SemVer::new(version, Options::EMPTY)
    }

    /// Recompute and return `this.version`.
    pub fn format(&mut self) -> String {
        let mut version = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if !self.prerelease.is_empty() {
            version.push('-');
            version.push_str(&join_identifiers(&self.prerelease));
        }
        self.version = version;
        self.version.clone()
    }

    /// The formatted version including build metadata (what `raw` becomes
    /// after `inc`).
    pub fn to_string_with_build(&self) -> String {
        if self.build.is_empty() {
            self.version.clone()
        } else {
            format!("{}+{}", self.version, self.build.join("."))
        }
    }

    /// `SemVer#compare`
    pub fn compare(&self, other: &SemVer) -> Ordering {
        if other.version == self.version {
            return Ordering::Equal;
        }
        match self.compare_main(other) {
            Ordering::Equal => self.compare_pre(other),
            ord => ord,
        }
    }

    /// `SemVer#compareMain`
    pub fn compare_main(&self, other: &SemVer) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }

    /// `SemVer#comparePre`
    pub fn compare_pre(&self, other: &SemVer) -> Ordering {
        // NOT having a prerelease is > having one
        if !self.prerelease.is_empty() && other.prerelease.is_empty() {
            return Ordering::Less;
        } else if self.prerelease.is_empty() && !other.prerelease.is_empty() {
            return Ordering::Greater;
        } else if self.prerelease.is_empty() && other.prerelease.is_empty() {
            return Ordering::Equal;
        }

        let mut i = 0usize;
        loop {
            match (self.prerelease.get(i), other.prerelease.get(i)) {
                (None, None) => return Ordering::Equal,
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (Some(a), Some(b)) => {
                    if a == b {
                        i += 1;
                        continue;
                    }
                    return compare_identifiers(a, b);
                }
            }
        }
    }

    /// `SemVer#compareBuild`
    pub fn compare_build(&self, other: &SemVer) -> Ordering {
        let mut i = 0usize;
        loop {
            match (self.build.get(i), other.build.get(i)) {
                (None, None) => return Ordering::Equal,
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (Some(a), Some(b)) => {
                    if a == b {
                        i += 1;
                        continue;
                    }
                    return compare_identifiers_str(a, b);
                }
            }
        }
    }

    /// `SemVer#inc`
    ///
    /// `premajor` etc. bump to the next release and immediately down to a
    /// prerelease of it.
    pub fn inc(
        &mut self,
        release: &str,
        identifier: Option<&str>,
        identifier_base: IdentifierBase,
    ) -> Result<&mut Self> {
        // `!identifier` is also true for the empty string.
        let identifier = identifier.filter(|s| !s.is_empty());

        if release.starts_with("pre") {
            if identifier.is_none() && identifier_base.is_false() {
                return Err(SemverError::InvalidIncrement(
                    "identifier is empty".to_string(),
                ));
            }
            if let Some(id) = identifier {
                let re = if self.options.loose {
                    safe_re(t::PRERELEASELOOSE)
                } else {
                    safe_re(t::PRERELEASE)
                };
                let probe = format!("-{id}");
                let ok = re
                    .captures(&probe)
                    .and_then(|m| m.get(1).map(|g| g.as_str() == id))
                    .unwrap_or(false);
                if !ok {
                    return Err(SemverError::InvalidIdentifier(id.to_string()));
                }
            }
        }

        match release {
            "premajor" => {
                self.prerelease.clear();
                self.patch = 0;
                self.minor = 0;
                self.major += 1;
                self.inc("pre", identifier, identifier_base)?;
            }
            "preminor" => {
                self.prerelease.clear();
                self.patch = 0;
                self.minor += 1;
                self.inc("pre", identifier, identifier_base)?;
            }
            "prepatch" => {
                // Drop any prereleases that might already exist, they are not
                // relevant at this point.
                self.prerelease.clear();
                self.inc("patch", identifier, identifier_base)?;
                self.inc("pre", identifier, identifier_base)?;
            }
            "prerelease" => {
                if self.prerelease.is_empty() {
                    self.inc("patch", identifier, identifier_base)?;
                }
                self.inc("pre", identifier, identifier_base)?;
            }
            "release" => {
                if self.prerelease.is_empty() {
                    return Err(SemverError::NotAPrerelease(self.raw.clone()));
                }
                self.prerelease.clear();
            }
            "major" => {
                // 1.0.0-5 bumps to 1.0.0; 1.1.0 bumps to 2.0.0
                if self.minor != 0 || self.patch != 0 || self.prerelease.is_empty() {
                    self.major += 1;
                }
                self.minor = 0;
                self.patch = 0;
                self.prerelease = Vec::new();
            }
            "minor" => {
                if self.patch != 0 || self.prerelease.is_empty() {
                    self.minor += 1;
                }
                self.patch = 0;
                self.prerelease = Vec::new();
            }
            "patch" => {
                if self.prerelease.is_empty() {
                    self.patch += 1;
                }
                self.prerelease = Vec::new();
            }
            "pre" => {
                let base = identifier_base.base();

                if self.prerelease.is_empty() {
                    self.prerelease = vec![Identifier::Numeric(base)];
                } else {
                    let mut incremented = false;
                    let mut i = self.prerelease.len();
                    while i > 0 {
                        i -= 1;
                        if let Identifier::Numeric(n) = &mut self.prerelease[i] {
                            *n += 1;
                            incremented = true;
                            break;
                        }
                    }
                    if !incremented {
                        // didn't increment anything
                        let joined = join_identifiers(&self.prerelease);
                        if identifier == Some(joined.as_str()) && identifier_base.is_false() {
                            return Err(SemverError::InvalidIncrement(
                                "identifier already exists".to_string(),
                            ));
                        }
                        self.prerelease.push(Identifier::Numeric(base));
                    }
                }

                if let Some(id) = identifier {
                    // 1.2.0-beta.1 bumps to 1.2.0-beta.2,
                    // 1.2.0-beta.fooblz or 1.2.0-beta bumps to 1.2.0-beta.0
                    let prerelease: Vec<Identifier> = if identifier_base.is_false() {
                        vec![Identifier::Alpha(id.to_string())]
                    } else {
                        vec![Identifier::Alpha(id.to_string()), Identifier::Numeric(base)]
                    };

                    if is_prerelease_identifier(&self.prerelease, id) {
                        let idx = id.split('.').count();
                        if is_nan_at(&self.prerelease, idx) {
                            self.prerelease = prerelease;
                        }
                    } else {
                        self.prerelease = prerelease;
                    }
                }
            }
            other => {
                return Err(SemverError::InvalidIncrement(other.to_string()));
            }
        }

        self.raw = self.format();
        if !self.build.is_empty() {
            self.raw = format!("{}+{}", self.raw, self.build.join("."));
        }
        Ok(self)
    }
}

/// `+m[n]` with the `> MAX_SAFE_INTEGER || < 0` guard. Overflowing `u64` also
/// means "past MAX_SAFE_INTEGER" so it maps to the same error.
fn parse_component(s: &str) -> Option<u64> {
    match s.parse::<u64>() {
        Ok(n) if n <= MAX_SAFE_INTEGER => Some(n),
        _ => None,
    }
}

pub(crate) fn join_identifiers(ids: &[Identifier]) -> String {
    let mut out = String::new();
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            out.push('.');
        }
        out.push_str(&id.as_str());
    }
    out
}

/// `isPrereleaseIdentifier(prerelease, identifier)`
fn is_prerelease_identifier(prerelease: &[Identifier], identifier: &str) -> bool {
    let identifiers: Vec<&str> = identifier.split('.').collect();
    if identifiers.len() > prerelease.len() {
        return false;
    }
    for (i, part) in identifiers.iter().enumerate() {
        if compare_identifiers(&prerelease[i], &Identifier::Alpha((*part).to_string()))
            != Ordering::Equal
        {
            return false;
        }
    }
    true
}

/// `isNaN(this.prerelease[idx])` — `undefined` and non-numeric strings are NaN.
fn is_nan_at(prerelease: &[Identifier], idx: usize) -> bool {
    match prerelease.get(idx) {
        None => true,
        Some(Identifier::Numeric(_)) => false,
        Some(Identifier::Alpha(s)) => js_number(s).is_none(),
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.version)
    }
}

impl PartialEq for SemVer {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}

impl Eq for SemVer {}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl std::str::FromStr for SemVer {
    type Err = SemverError;

    fn from_str(s: &str) -> Result<SemVer> {
        SemVer::new(s, Options::EMPTY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full() {
        let v = SemVer::parse("1.2.3-alpha.1+build.7").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(
            v.prerelease,
            vec![Identifier::Alpha("alpha".into()), Identifier::Numeric(1)]
        );
        assert_eq!(v.build, vec!["build".to_string(), "7".to_string()]);
        assert_eq!(v.version, "1.2.3-alpha.1");
    }

    #[test]
    fn loose_only_in_loose_mode() {
        assert!(SemVer::parse("=v1.2.3").is_err());
        assert!(SemVer::new("=v1.2.3", Options::LOOSE).is_ok());
    }

    #[test]
    fn invalid_version_message() {
        let err = SemVer::parse("not a version").unwrap_err();
        assert_eq!(err.to_string(), "Invalid Version: not a version");
    }

    #[test]
    fn prerelease_ordering() {
        let a = SemVer::parse("1.0.0-alpha").unwrap();
        let b = SemVer::parse("1.0.0-alpha.1").unwrap();
        let c = SemVer::parse("1.0.0").unwrap();
        assert_eq!(a.compare(&b), Ordering::Less);
        assert_eq!(b.compare(&c), Ordering::Less);
        assert_eq!(c.compare(&a), Ordering::Greater);
    }

    #[test]
    fn js_number_stringification() {
        assert_eq!(js_number_to_string(3.0), "3");
        assert_eq!(js_number_to_string(0.0), "0");
        assert_eq!(js_number_to_string(1e20), "100000000000000000000");
        // JS switches to exponential notation at 1e21.
        assert_eq!(js_number_to_string(1e21), "1e+21");
        assert_eq!(js_number_to_string(1e25), "1e+25");
        assert_eq!(js_number_to_string(1.5e25), "1.5e+25");
        assert_eq!(js_number_to_string(f64::INFINITY), "Infinity");
        assert_eq!(js_number_to_string(f64::NAN), "NaN");
    }

    #[test]
    fn max_length_is_enforced_before_parsing() {
        let long = format!("1.2.3-{}", "a".repeat(300));
        let err = SemVer::parse(&long).unwrap_err();
        assert_eq!(err.to_string(), "version is longer than 256 characters");
    }

    #[test]
    fn components_past_max_safe_integer_are_rejected() {
        assert!(SemVer::parse("9007199254740991.0.0").is_ok());
        assert_eq!(
            SemVer::parse("9007199254740992.0.0").unwrap_err().to_string(),
            "Invalid major version"
        );
        // A numeric prerelease past the limit stays a string instead.
        let v = SemVer::parse("1.2.3-9007199254740992").unwrap();
        assert_eq!(
            v.prerelease,
            vec![Identifier::Alpha("9007199254740992".into())]
        );
    }

    #[test]
    fn build_metadata_is_ignored_by_compare_but_not_compare_build() {
        let a = SemVer::parse("1.2.3+a").unwrap();
        let b = SemVer::parse("1.2.3+b").unwrap();
        assert_eq!(a.compare(&b), Ordering::Equal);
        assert_eq!(a.compare_build(&b), Ordering::Less);
    }

    #[test]
    fn inc_basics() {
        let mut v = SemVer::parse("1.2.3").unwrap();
        v.inc("major", None, IdentifierBase::Unset).unwrap();
        assert_eq!(v.version, "2.0.0");

        let mut v = SemVer::parse("1.2.3").unwrap();
        v.inc("prerelease", Some("beta"), IdentifierBase::Unset)
            .unwrap();
        assert_eq!(v.version, "1.2.4-beta.0");

        let mut v = SemVer::parse("1.2.4-beta.0").unwrap();
        v.inc("prerelease", Some("beta"), IdentifierBase::Unset)
            .unwrap();
        assert_eq!(v.version, "1.2.4-beta.1");
    }
}
