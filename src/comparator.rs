//! Port of `classes/comparator.js`.
//!
//! The `ANY` sentinel (a `Symbol` in JavaScript) is modelled as
//! `Comparator::semver == None`.

use std::fmt;

use crate::error::{Result, SemverError};
use crate::functions::cmp_semver;
use crate::options::Options;
use crate::range::{Range, ToRange};
use crate::re::{safe_re, t, SPACE_CHARACTERS};
use crate::semver::SemVer;

#[derive(Debug, Clone)]
pub struct Comparator {
    pub operator: String,
    /// `None` is `Comparator.ANY`.
    pub semver: Option<SemVer>,
    pub value: String,
    pub options: Options,
    pub loose: bool,
}

impl Comparator {
    pub fn new(comp: &str, options: Options) -> Result<Comparator> {
        // `comp.trim().split(/\s+/).join(' ')`
        let normalized = SPACE_CHARACTERS.replace_all(comp.trim(), " ").into_owned();

        let (operator, semver) = Comparator::parse_parts(&normalized, options)?;

        let value = match &semver {
            None => String::new(),
            Some(sv) => format!("{}{}", operator, sv.version),
        };

        Ok(Comparator {
            operator,
            semver,
            value,
            options,
            loose: options.loose,
        })
    }

    /// `Comparator#parse`
    fn parse_parts(comp: &str, options: Options) -> Result<(String, Option<SemVer>)> {
        let r = if options.loose {
            safe_re(t::COMPARATORLOOSE)
        } else {
            safe_re(t::COMPARATOR)
        };

        let m = match r.captures(comp) {
            Some(m) => m,
            None => return Err(SemverError::InvalidComparator(comp.to_string())),
        };

        let mut operator = m.get(1).map(|g| g.as_str()).unwrap_or("").to_string();
        if operator == "=" {
            operator.clear();
        }

        // if it literally is just '>' or '' then allow anything.
        let semver = match m.get(2) {
            None => None,
            Some(v) if v.as_str().is_empty() => None,
            // `new SemVer(m[2], this.options.loose)`: only the loose flag is
            // forwarded, matching `parseOptions(boolean)`.
            Some(v) => Some(SemVer::new(v.as_str(), options.only_loose())?),
        };

        Ok((operator, semver))
    }

    /// `this.semver === ANY`
    pub fn is_any(&self) -> bool {
        self.semver.is_none()
    }

    /// The `<0.0.0-0` null set.
    pub fn is_null_set(&self) -> bool {
        self.value == "<0.0.0-0"
    }

    /// `Comparator#test` for an already-parsed version.
    pub fn test_semver(&self, version: &SemVer) -> bool {
        match &self.semver {
            None => true,
            Some(sv) => cmp_semver(version, &self.operator, sv, self.options).unwrap_or(false),
        }
    }

    /// `Comparator#test` for a string; an unparseable version is `false`.
    pub fn test(&self, version: &str) -> bool {
        if self.semver.is_none() {
            return true;
        }
        match SemVer::new(version, self.options) {
            Ok(v) => self.test_semver(&v),
            Err(_) => false,
        }
    }

    /// `Comparator#intersects`
    pub fn intersects(&self, comp: &Comparator, options: Options) -> Result<bool> {
        if self.operator.is_empty() {
            if self.value.is_empty() {
                return Ok(true);
            }
            return Ok(Range::new(&comp.value, options)?.test(&self.value));
        } else if comp.operator.is_empty() {
            if comp.value.is_empty() {
                return Ok(true);
            }
            let range = Range::new(&self.value, options)?;
            return Ok(match &comp.semver {
                None => true,
                Some(sv) => range.test_semver(sv),
            });
        }

        // Special cases where nothing can possibly be lower
        if options.include_prerelease
            && (self.value == "<0.0.0-0" || comp.value == "<0.0.0-0")
        {
            return Ok(false);
        }
        if !options.include_prerelease
            && (self.value.starts_with("<0.0.0") || comp.value.starts_with("<0.0.0"))
        {
            return Ok(false);
        }

        // Same direction increasing (> or >=)
        if self.operator.starts_with('>') && comp.operator.starts_with('>') {
            return Ok(true);
        }
        // Same direction decreasing (< or <=)
        if self.operator.starts_with('<') && comp.operator.starts_with('<') {
            return Ok(true);
        }

        let (a, b) = match (&self.semver, &comp.semver) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(true),
        };

        // same SemVer and both sides are inclusive (<= or >=)
        if a.version == b.version
            && self.operator.contains('=')
            && comp.operator.contains('=')
        {
            return Ok(true);
        }
        // opposite directions less than
        if cmp_semver(a, "<", b, options)?
            && self.operator.starts_with('>')
            && comp.operator.starts_with('<')
        {
            return Ok(true);
        }
        // opposite directions greater than
        if cmp_semver(a, ">", b, options)?
            && self.operator.starts_with('<')
            && comp.operator.starts_with('>')
        {
            return Ok(true);
        }
        Ok(false)
    }
}

impl fmt::Display for Comparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

impl ToRange for Comparator {
    fn to_range(&self, _options: Options) -> Result<Range> {
        Ok(Range::from_comparator(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_comparator() {
        let c = Comparator::new("", Options::EMPTY).unwrap();
        assert!(c.is_any());
        assert_eq!(c.value, "");
        assert!(c.test("1.2.3"));
    }

    #[test]
    fn operators() {
        let c = Comparator::new(">=1.2.3", Options::EMPTY).unwrap();
        assert_eq!(c.operator, ">=");
        assert_eq!(c.value, ">=1.2.3");
        assert!(c.test("1.2.3"));
        assert!(c.test("2.0.0"));
        assert!(!c.test("1.2.2"));
    }

    #[test]
    fn equals_operator_is_stripped() {
        let c = Comparator::new("=1.2.3", Options::EMPTY).unwrap();
        assert_eq!(c.operator, "");
        assert_eq!(c.value, "1.2.3");
    }

    #[test]
    fn invalid_comparator_message() {
        let err = Comparator::new("foo bar baz", Options::EMPTY).unwrap_err();
        assert_eq!(err.to_string(), "Invalid comparator: foo bar baz");
    }
}
