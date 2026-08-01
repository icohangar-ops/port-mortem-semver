//! Port of `internal/identifiers.js`.
//!
//! In JavaScript a prerelease component is either a `number` (when the source
//! text is a run of digits that fits in `MAX_SAFE_INTEGER`) or a `string`.
//! [`Identifier`] models that union explicitly.

use std::cmp::Ordering;
use std::fmt;

use crate::constants::MAX_SAFE_INTEGER;

/// `const numeric = /^[0-9]+$/`
#[inline]
pub fn is_numeric_str(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[derive(Debug, Clone, Eq)]
pub enum Identifier {
    /// A JS `number` prerelease component.
    Numeric(u64),
    /// A JS `string` prerelease component.
    Alpha(String),
}

impl Identifier {
    /// The `m[4].split('.').map(...)` numberification rule from `SemVer`'s
    /// constructor: digits-only and `>= 0 && < MAX_SAFE_INTEGER` become numbers.
    pub fn parse(id: &str) -> Identifier {
        if is_numeric_str(id) {
            if let Ok(num) = id.parse::<u64>() {
                if num < MAX_SAFE_INTEGER {
                    return Identifier::Numeric(num);
                }
            }
        }
        Identifier::Alpha(id.to_string())
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Identifier::Numeric(_))
    }

    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Identifier::Numeric(n) => std::borrow::Cow::Owned(n.to_string()),
            Identifier::Alpha(s) => std::borrow::Cow::Borrowed(s.as_str()),
        }
    }

    /// `+value` when the value is digits-only, else `None`. Numbers past
    /// `MAX_SAFE_INTEGER` are stored as strings but JS still coerces them with
    /// `+`, hence the `f64`.
    fn numeric_value(&self) -> Option<f64> {
        match self {
            Identifier::Numeric(n) => Some(*n as f64),
            Identifier::Alpha(s) => {
                if is_numeric_str(s) {
                    s.parse::<f64>().ok()
                } else {
                    None
                }
            }
        }
    }
}

impl PartialEq for Identifier {
    /// Mirrors JS `a === b`: a number is never `===` a string.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Identifier::Numeric(a), Identifier::Numeric(b)) => a == b,
            (Identifier::Alpha(a), Identifier::Alpha(b)) => a == b,
            _ => false,
        }
    }
}

/// Written out by hand (rather than derived) to keep it visibly in step with
/// the custom `PartialEq`: a number and a string never compare equal, so they
/// must also hash into distinct buckets.
impl std::hash::Hash for Identifier {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Identifier::Numeric(n) => {
                0u8.hash(state);
                n.hash(state);
            }
            Identifier::Alpha(s) => {
                1u8.hash(state);
                s.hash(state);
            }
        }
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Identifier::Numeric(n) => write!(f, "{n}"),
            Identifier::Alpha(s) => write!(f, "{s}"),
        }
    }
}

impl From<u64> for Identifier {
    fn from(n: u64) -> Self {
        Identifier::Numeric(n)
    }
}

impl From<&str> for Identifier {
    fn from(s: &str) -> Self {
        Identifier::Alpha(s.to_string())
    }
}

impl From<String> for Identifier {
    fn from(s: String) -> Self {
        Identifier::Alpha(s)
    }
}

/// `compareIdentifiers(a, b)`
pub fn compare_identifiers(a: &Identifier, b: &Identifier) -> Ordering {
    match (a.numeric_value(), b.numeric_value()) {
        (Some(x), Some(y)) => {
            if x == y {
                Ordering::Equal
            } else if x < y {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        // anum && !bnum
        (Some(_), None) => Ordering::Less,
        // bnum && !anum
        (None, Some(_)) => Ordering::Greater,
        (None, None) => {
            let (x, y) = (a.as_str(), b.as_str());
            if x == y {
                Ordering::Equal
            } else if x < y {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

/// `rcompareIdentifiers(a, b)`
pub fn rcompare_identifiers(a: &Identifier, b: &Identifier) -> Ordering {
    compare_identifiers(b, a)
}

/// String-only flavour, matching the exported `compareIdentifiers` when both
/// arguments come in as strings (this is how build metadata is compared).
pub fn compare_identifiers_str(a: &str, b: &str) -> Ordering {
    let anum = is_numeric_str(a);
    let bnum = is_numeric_str(b);

    if anum && bnum {
        let (x, y) = (
            a.parse::<f64>().unwrap_or(f64::NAN),
            b.parse::<f64>().unwrap_or(f64::NAN),
        );
        return if x == y {
            Ordering::Equal
        } else if x < y {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }

    if a == b {
        Ordering::Equal
    } else if anum && !bnum {
        Ordering::Less
    } else if bnum && !anum {
        Ordering::Greater
    } else if a < b {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

pub fn rcompare_identifiers_str(a: &str, b: &str) -> Ordering {
    compare_identifiers_str(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_beats_alpha() {
        assert_eq!(compare_identifiers_str("1", "a"), Ordering::Less);
        assert_eq!(compare_identifiers_str("a", "1"), Ordering::Greater);
        assert_eq!(compare_identifiers_str("1", "1"), Ordering::Equal);
        assert_eq!(compare_identifiers_str("01", "1"), Ordering::Equal);
        assert_eq!(compare_identifiers_str("2", "10"), Ordering::Less);
        assert_eq!(compare_identifiers_str("a", "b"), Ordering::Less);
    }

    #[test]
    fn identifier_parse() {
        assert_eq!(Identifier::parse("0"), Identifier::Numeric(0));
        assert_eq!(Identifier::parse("alpha"), Identifier::Alpha("alpha".into()));
        // Beyond MAX_SAFE_INTEGER it stays a string.
        assert_eq!(
            Identifier::parse("9007199254740991"),
            Identifier::Alpha("9007199254740991".into())
        );
    }
}
