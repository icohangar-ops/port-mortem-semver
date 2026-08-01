//! Ports of everything under `functions/`.

use std::cmp::Ordering;

use crate::constants::RELEASE_TYPES;
use crate::error::{Result, SemverError};
use crate::identifiers::Identifier;
use crate::options::Options;
use crate::range::{Range, ToRange};
use crate::re::{safe_re, t};
use crate::semver::{IdentifierBase, SemVer, ToSemVer};

/// `parse(version, options)` — `None` instead of `null`.
pub fn parse(version: &str, options: Options) -> Option<SemVer> {
    SemVer::new(version, options).ok()
}

/// `parse(version, options, true)` — the throwing flavour used by `diff`.
pub fn parse_throw(version: &str, options: Options) -> Result<SemVer> {
    SemVer::new(version, options)
}

/// `valid(version, options)`
pub fn valid(version: &str, options: Options) -> Option<String> {
    parse(version, options).map(|v| v.version)
}

/// `clean(version, options)`
pub fn clean(version: &str, options: Options) -> Option<String> {
    let trimmed = version.trim();
    let stripped = trimmed.trim_start_matches(['=', 'v']);
    parse(stripped, options).map(|v| v.version)
}

/// `inc(version, release, options, identifier, identifierBase)`
///
/// Any error becomes `None`, exactly like the JS `try/catch`.
pub fn inc(
    version: &str,
    release: &str,
    options: Options,
    identifier: Option<&str>,
    identifier_base: IdentifierBase,
) -> Option<String> {
    let mut sv = SemVer::new(version, options).ok()?;
    sv.inc(release, identifier, identifier_base).ok()?;
    Some(sv.version)
}

/// `diff(version1, version2)` — note that JS ignores options here and always
/// parses strictly, throwing on invalid input.
pub fn diff(version1: &str, version2: &str) -> Result<Option<String>> {
    let v1 = parse_throw(version1, Options::EMPTY)?;
    let v2 = parse_throw(version2, Options::EMPTY)?;
    let comparison = v1.compare(&v2);

    if comparison == Ordering::Equal {
        return Ok(None);
    }

    let v1_higher = comparison == Ordering::Greater;
    let high_version = if v1_higher { &v1 } else { &v2 };
    let low_version = if v1_higher { &v2 } else { &v1 };
    let high_has_pre = !high_version.prerelease.is_empty();
    let low_has_pre = !low_version.prerelease.is_empty();

    if low_has_pre && !high_has_pre {
        // Going from prerelease -> no prerelease requires some special casing.

        // If the low version has only a major, then it will always be a major.
        if low_version.patch == 0 && low_version.minor == 0 {
            return Ok(Some("major".to_string()));
        }

        // If the main part has no difference
        if low_version.compare_main(high_version) == Ordering::Equal {
            if low_version.minor != 0 && low_version.patch == 0 {
                return Ok(Some("minor".to_string()));
            }
            return Ok(Some("patch".to_string()));
        }
    }

    // add the `pre` prefix if we are going to a prerelease version
    let prefix = if high_has_pre { "pre" } else { "" };

    if v1.major != v2.major {
        return Ok(Some(format!("{prefix}major")));
    }
    if v1.minor != v2.minor {
        return Ok(Some(format!("{prefix}minor")));
    }
    if v1.patch != v2.patch {
        return Ok(Some(format!("{prefix}patch")));
    }

    // high and low are prereleases
    Ok(Some("prerelease".to_string()))
}

pub fn major<A: ToSemVer + ?Sized>(a: &A, options: Options) -> Result<u64> {
    Ok(a.to_semver(options)?.major)
}

pub fn minor<A: ToSemVer + ?Sized>(a: &A, options: Options) -> Result<u64> {
    Ok(a.to_semver(options)?.minor)
}

pub fn patch<A: ToSemVer + ?Sized>(a: &A, options: Options) -> Result<u64> {
    Ok(a.to_semver(options)?.patch)
}

/// `prerelease(version, options)` — `None` when absent or unparseable.
pub fn prerelease(version: &str, options: Options) -> Option<Vec<Identifier>> {
    let parsed = parse(version, options)?;
    if parsed.prerelease.is_empty() {
        None
    } else {
        Some(parsed.prerelease)
    }
}

pub fn compare<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<Ordering> {
    Ok(a.to_semver(options)?.compare(&b.to_semver(options)?))
}

pub fn rcompare<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<Ordering> {
    compare(b, a, options)
}

pub fn compare_loose<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(a: &A, b: &B) -> Result<Ordering> {
    compare(a, b, Options::LOOSE)
}

/// `compareBuild(a, b, loose)`
pub fn compare_build<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<Ordering> {
    let version_a = a.to_semver(options)?;
    let version_b = b.to_semver(options)?;
    Ok(match version_a.compare(&version_b) {
        Ordering::Equal => version_a.compare_build(&version_b),
        ord => ord,
    })
}

/// `sort(list, loose)` — stable, ordered by `compareBuild`.
pub fn sort(list: Vec<String>, options: Options) -> Result<Vec<String>> {
    sort_with(list, options, false)
}

/// `rsort(list, loose)`
pub fn rsort(list: Vec<String>, options: Options) -> Result<Vec<String>> {
    sort_with(list, options, true)
}

fn sort_with(list: Vec<String>, options: Options, reverse: bool) -> Result<Vec<String>> {
    if list.len() < 2 {
        // `Array#sort` never invokes the comparator, so nothing is validated.
        return Ok(list);
    }
    let mut pairs = parse_for_sort(list, options, reverse)?;
    pairs.sort_by(|a, b| {
        let ord = match a.0.compare(&b.0) {
            Ordering::Equal => a.0.compare_build(&b.0),
            o => o,
        };
        if reverse {
            ord.reverse()
        } else {
            ord
        }
    });
    Ok(pairs.into_iter().map(|(_, v)| v).collect())
}

/// Parse a whole list up front so the sort comparator itself can be infallible.
///
/// JavaScript instead throws from *inside* `Array#sort`, so the version named in
/// the error is whichever one the comparator happened to touch first rather than
/// the first invalid element of the list. Two consequences are reproduced here:
/// a list shorter than two entries is never validated at all (no comparison ever
/// runs), and the first two elements are checked in comparator order —
/// `sort` compares `(list[1], list[0])` and parses its left argument first,
/// while `rsort` flips the arguments.
pub(crate) fn parse_for_sort(
    list: Vec<String>,
    options: Options,
    reverse: bool,
) -> Result<Vec<(SemVer, String)>> {
    debug_assert!(list.len() >= 2, "callers short-circuit shorter lists");

    let probe = if reverse { [0usize, 1] } else { [1usize, 0] };
    let mut parsed: Vec<Option<SemVer>> = vec![None; list.len()];
    for i in probe.into_iter().chain(2..list.len()) {
        parsed[i] = Some(SemVer::new(&list[i], options)?);
    }

    Ok(parsed
        .into_iter()
        .map(|p| p.expect("every index was parsed"))
        .zip(list)
        .collect())
}

pub fn gt<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? == Ordering::Greater)
}

pub fn lt<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? == Ordering::Less)
}

pub fn eq<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? == Ordering::Equal)
}

pub fn neq<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? != Ordering::Equal)
}

pub fn gte<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? != Ordering::Less)
}

pub fn lte<A: ToSemVer + ?Sized, B: ToSemVer + ?Sized>(
    a: &A,
    b: &B,
    options: Options,
) -> Result<bool> {
    Ok(compare(a, b, options)? != Ordering::Greater)
}

/// `cmp(a, op, b, loose)` for string operands.
pub fn cmp(a: &str, op: &str, b: &str, options: Options) -> Result<bool> {
    match op {
        "===" => Ok(a == b),
        "!==" => Ok(a != b),
        "" | "=" | "==" => eq(a, b, options),
        "!=" => neq(a, b, options),
        ">" => gt(a, b, options),
        ">=" => gte(a, b, options),
        "<" => lt(a, b, options),
        "<=" => lte(a, b, options),
        _ => Err(SemverError::InvalidOperator(op.to_string())),
    }
}

/// `cmp` where both operands are already `SemVer` objects (the identity
/// operators fall back to comparing `.version`, as in JS).
pub fn cmp_semver(a: &SemVer, op: &str, b: &SemVer, options: Options) -> Result<bool> {
    let _ = options;
    match op {
        "===" => Ok(a.version == b.version),
        "!==" => Ok(a.version != b.version),
        "" | "=" | "==" => Ok(a.compare(b) == Ordering::Equal),
        "!=" => Ok(a.compare(b) != Ordering::Equal),
        ">" => Ok(a.compare(b) == Ordering::Greater),
        ">=" => Ok(a.compare(b) != Ordering::Less),
        "<" => Ok(a.compare(b) == Ordering::Less),
        "<=" => Ok(a.compare(b) != Ordering::Greater),
        _ => Err(SemverError::InvalidOperator(op.to_string())),
    }
}

/// `coerce(version, options)`
pub fn coerce(version: &str, options: Options) -> Option<SemVer> {
    let (major, minor, patch, pre, build) = if !options.rtl {
        let r = if options.include_prerelease {
            safe_re(t::COERCEFULL)
        } else {
            safe_re(t::COERCE)
        };
        let m = r.captures(version)?;
        extract_coerce(&m, options)
    } else {
        // Find the right-most coercible string that does not share a terminus
        // with a more left-ward coercible string. Eg, '1.2.3.4' wants to coerce
        // '2.3.4', not '3.4' or '4'.
        let r = if options.include_prerelease {
            safe_re(t::COERCERTLFULL)
        } else {
            safe_re(t::COERCERTL)
        };

        let mut best: Option<(usize, usize)> = None; // (start, end) of match 0
        let mut best_caps = None;
        let mut last_index = 0usize;

        loop {
            if last_index > version.len() || !version.is_char_boundary(last_index) {
                break;
            }
            let next = match r.captures_at(version, last_index) {
                Some(c) => c,
                None => break,
            };
            // The `while` condition stops once the current best match ends at
            // the end of the string.
            if let Some((_, end)) = best {
                if end == version.len() {
                    break;
                }
            }

            let m0 = next.get(0).unwrap();
            let (start, end) = (m0.start(), m0.end());
            let g1 = next.get(1).map(|g| g.len()).unwrap_or(0);
            let g2 = next.get(2).map(|g| g.len()).unwrap_or(0);

            let replace = match best {
                None => true,
                Some((_, best_end)) => end != best_end,
            };
            if replace {
                best = Some((start, end));
                best_caps = Some(next);
            }
            last_index = start + g1 + g2;
        }

        let m = best_caps?;
        extract_coerce(&m, options)
    };

    let combined = format!("{major}.{minor}.{patch}{pre}{build}");
    parse(&combined, options)
}

fn extract_coerce(
    m: &regex::Captures<'_>,
    options: Options,
) -> (String, String, String, String, String) {
    let major = m.get(2).map(|g| g.as_str()).unwrap_or("").to_string();
    let minor = m
        .get(3)
        .map(|g| g.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("0")
        .to_string();
    let patch = m
        .get(4)
        .map(|g| g.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("0")
        .to_string();
    let pre = if options.include_prerelease {
        m.get(5)
            .map(|g| g.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| format!("-{s}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let build = if options.include_prerelease {
        m.get(6)
            .map(|g| g.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| format!("+{s}"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    (major, minor, patch, pre, build)
}

/// `truncate(version, truncation, options)`
pub fn truncate(version: &str, truncation: &str, options: Options) -> Option<String> {
    if !RELEASE_TYPES.contains(&truncation) {
        return None;
    }

    let mut cloned = parse(version, options)?;

    if truncation.starts_with("pre") {
        return Some(cloned.version);
    }

    cloned.prerelease.clear();
    match truncation {
        "major" => {
            cloned.minor = 0;
            cloned.patch = 0;
        }
        "minor" => {
            cloned.patch = 0;
        }
        _ => {}
    }

    Some(cloned.format())
}

/// `satisfies(version, range, options)`
pub fn satisfies<R: ToRange + ?Sized>(version: &str, range: &R, options: Options) -> bool {
    match range.to_range(options) {
        Ok(r) => r.test(version),
        Err(_) => false,
    }
}

/// `satisfies` for an already-parsed version.
pub fn satisfies_semver<R: ToRange + ?Sized>(
    version: &SemVer,
    range: &R,
    options: Options,
) -> bool {
    match range.to_range(options) {
        Ok(r) => r.test_semver(version),
        Err(_) => false,
    }
}

/// Convenience: build a [`Range`] the way `new Range(...)` would.
pub fn range<R: ToRange + ?Sized>(range: &R, options: Options) -> Result<Range> {
    range.to_range(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_and_clean() {
        assert_eq!(valid("1.2.3", Options::EMPTY).as_deref(), Some("1.2.3"));
        assert_eq!(valid("a.b.c", Options::EMPTY), None);
        assert_eq!(clean("  =v1.2.3   ", Options::EMPTY).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn comparisons() {
        assert!(gt("2.0.0", "1.9.9", Options::EMPTY).unwrap());
        assert!(lt("1.0.0-alpha", "1.0.0", Options::EMPTY).unwrap());
        assert!(eq("1.2.3+a", "1.2.3+b", Options::EMPTY).unwrap());
        assert_eq!(
            compare_build("1.2.3+a", "1.2.3+b", Options::EMPTY).unwrap(),
            Ordering::Less
        );
    }

    #[test]
    fn diffs() {
        assert_eq!(diff("1.2.3", "1.2.4").unwrap().as_deref(), Some("patch"));
        assert_eq!(diff("1.2.3", "2.0.0").unwrap().as_deref(), Some("major"));
        assert_eq!(diff("1.2.3", "1.2.3").unwrap(), None);
        assert_eq!(
            diff("1.0.0", "1.0.0-1").unwrap().as_deref(),
            Some("major")
        );
    }

    #[test]
    fn coercion() {
        assert_eq!(
            coerce("v2.x", Options::EMPTY).map(|v| v.version),
            Some("2.0.0".to_string())
        );
        assert_eq!(
            coerce("1.2.3.4", Options::EMPTY.rtl(true)).map(|v| v.version),
            Some("2.3.4".to_string())
        );
        assert_eq!(coerce("not a version", Options::EMPTY), None);
    }

    #[test]
    fn sorting() {
        let list = vec!["2.0.0".to_string(), "1.0.0".to_string(), "1.5.0".to_string()];
        assert_eq!(
            sort(list.clone(), Options::EMPTY).unwrap(),
            vec!["1.0.0", "1.5.0", "2.0.0"]
        );
        assert_eq!(
            rsort(list, Options::EMPTY).unwrap(),
            vec!["2.0.0", "1.5.0", "1.0.0"]
        );
    }

    #[test]
    fn sort_validation_matches_js_comparator_order() {
        // `Array#sort` never runs the comparator on a list this short, so an
        // invalid entry is returned untouched rather than throwing.
        let single = vec!["bogus".to_string()];
        assert_eq!(sort(single.clone(), Options::EMPTY).unwrap(), single);
        assert_eq!(rsort(single.clone(), Options::EMPTY).unwrap(), single);

        // With two invalid entries, `sort` compares (list[1], list[0]) and
        // parses its left argument first, so list[1] is the one reported.
        let both_bad = vec!["bad-a".to_string(), "bad-b".to_string()];
        assert_eq!(
            sort(both_bad.clone(), Options::EMPTY).unwrap_err().to_string(),
            "Invalid Version: bad-b"
        );
        // `rsort` flips the arguments, so list[0] is reported instead.
        assert_eq!(
            rsort(both_bad, Options::EMPTY).unwrap_err().to_string(),
            "Invalid Version: bad-a"
        );
    }

    #[test]
    fn sort_is_stable_on_build_metadata() {
        let list = vec![
            "1.2.3+b".to_string(),
            "1.2.3+a".to_string(),
            "1.2.3".to_string(),
        ];
        assert_eq!(
            sort(list, Options::EMPTY).unwrap(),
            vec!["1.2.3", "1.2.3+a", "1.2.3+b"]
        );
    }

    #[test]
    fn inc_identifier_base() {
        use crate::semver::IdentifierBase;
        let i = |v: &str, rel: &str, id: Option<&str>, b: IdentifierBase| {
            inc(v, rel, Options::EMPTY, id, b)
        };
        assert_eq!(
            i("1.2.3", "prerelease", Some("beta"), IdentifierBase::One).as_deref(),
            Some("1.2.4-beta.1")
        );
        assert_eq!(
            i("1.2.3", "prerelease", Some("beta"), IdentifierBase::False).as_deref(),
            Some("1.2.4-beta")
        );
        // `identifierBase: false` with no identifier is an error, hence None.
        assert_eq!(i("1.2.3", "prerelease", None, IdentifierBase::False), None);
        // Unknown release types and invalid identifiers are swallowed too.
        assert_eq!(i("1.2.3", "bogus", None, IdentifierBase::Unset), None);
        assert_eq!(i("1.2.3", "prerelease", Some("bad!"), IdentifierBase::Unset), None);
        // `release` requires an existing prerelease.
        assert_eq!(i("1.2.3", "release", None, IdentifierBase::Unset), None);
        assert_eq!(i("1.2.3-0", "release", None, IdentifierBase::Unset).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn coerce_rtl_prefers_rightmost_non_overlapping() {
        let rtl = Options::EMPTY.rtl(true);
        assert_eq!(coerce("1.2.3.4", rtl).map(|v| v.version).as_deref(), Some("2.3.4"));
        assert_eq!(coerce("1.2.3.4.5", rtl).map(|v| v.version).as_deref(), Some("3.4.5"));
        let full = Options::EMPTY.rtl(true).include_prerelease(true);
        assert_eq!(
            coerce("1.2.3.4-rc", full).map(|v| v.version).as_deref(),
            Some("2.3.4-rc")
        );
    }

    #[test]
    fn cmp_operators_and_errors() {
        assert!(cmp("1.2.3", "===", "1.2.3", Options::EMPTY).unwrap());
        // `===` is a raw string identity check, not a semver comparison.
        assert!(!cmp("v1.2.3", "===", "1.2.3", Options::EMPTY).unwrap());
        assert!(cmp("1.2.3", "", "1.2.3", Options::EMPTY).unwrap());
        assert_eq!(
            cmp("1.2.3", "=~", "1.2.3", Options::EMPTY).unwrap_err().to_string(),
            "Invalid operator: =~"
        );
    }

    #[test]
    fn truncate_rules() {
        let o = Options::EMPTY;
        assert_eq!(truncate("1.2.3-a+b", "major", o).as_deref(), Some("1.0.0"));
        assert_eq!(truncate("1.2.3-a+b", "minor", o).as_deref(), Some("1.2.0"));
        assert_eq!(truncate("1.2.3-a+b", "patch", o).as_deref(), Some("1.2.3"));
        // `pre*` truncations keep the prerelease as-is.
        assert_eq!(truncate("1.2.3-a+b", "prerelease", o).as_deref(), Some("1.2.3-a"));
        // Unknown truncations are null, and `release` is not a release type.
        assert_eq!(truncate("1.2.3", "release", o), None);
    }

    #[test]
    fn satisfies_basics() {
        assert!(satisfies("1.2.4", "^1.2.3", Options::EMPTY));
        assert!(!satisfies("2.0.0", "^1.2.3", Options::EMPTY));
        assert!(!satisfies("1.2.4", "not a range", Options::EMPTY));
    }
}
