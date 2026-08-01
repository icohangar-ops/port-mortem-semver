//! Ports of everything under `ranges/`.

use std::cmp::Ordering;
use std::ptr;

use crate::comparator::Comparator;
use crate::error::{Result, SemverError};
use crate::functions::{compare, gt, gte, lt, lte, satisfies_semver};
use crate::identifiers::Identifier;
use crate::options::Options;
use crate::range::{Range, ToRange};
use crate::semver::SemVer;

/// `toComparators(range, options)` — mostly for testing and legacy API reasons.
pub fn to_comparators<R: ToRange + ?Sized>(
    range: &R,
    options: Options,
) -> Result<Vec<Vec<String>>> {
    let range = range.to_range(options)?;
    Ok(range
        .set
        .iter()
        .map(|comps| {
            let joined = comps
                .iter()
                .map(|c| c.value.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            joined
                .trim()
                .split(' ')
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
        .collect())
}

/// `maxSatisfying(versions, range, options)`
pub fn max_satisfying(versions: &[String], range: &str, options: Options) -> Option<String> {
    let range_obj = Range::new(range, options).ok()?;

    let mut max: Option<String> = None;
    let mut max_sv: Option<SemVer> = None;

    for v in versions {
        if !range_obj.test(v) {
            continue;
        }
        let candidate = match SemVer::new(v, options) {
            Ok(sv) => sv,
            Err(_) => continue,
        };
        let better = match &max_sv {
            None => true,
            Some(cur) => cur.compare(&candidate) == Ordering::Less,
        };
        if max.is_none() || better {
            max = Some(v.clone());
            max_sv = Some(candidate);
        }
    }

    max
}

/// `minSatisfying(versions, range, options)`
pub fn min_satisfying(versions: &[String], range: &str, options: Options) -> Option<String> {
    let range_obj = Range::new(range, options).ok()?;

    let mut min: Option<String> = None;
    let mut min_sv: Option<SemVer> = None;

    for v in versions {
        if !range_obj.test(v) {
            continue;
        }
        let candidate = match SemVer::new(v, options) {
            Ok(sv) => sv,
            Err(_) => continue,
        };
        let better = match &min_sv {
            None => true,
            Some(cur) => cur.compare(&candidate) == Ordering::Greater,
        };
        if min.is_none() || better {
            min = Some(v.clone());
            min_sv = Some(candidate);
        }
    }

    min
}

/// `minVersion(range, loose)` — the lowest version that can satisfy the range.
pub fn min_version<R: ToRange + ?Sized>(range: &R, options: Options) -> Result<Option<SemVer>> {
    let range = range.to_range(options)?;

    let zero = SemVer::new("0.0.0", Options::EMPTY)?;
    if range.test_semver(&zero) {
        return Ok(Some(zero));
    }

    let zero_pre = SemVer::new("0.0.0-0", Options::EMPTY)?;
    if range.test_semver(&zero_pre) {
        return Ok(Some(zero_pre));
    }

    let mut minver: Option<SemVer> = None;

    for comparators in &range.set {
        let mut set_min: Option<SemVer> = None;

        for comparator in comparators {
            let semver = match &comparator.semver {
                // `*` alone is already covered by the 0.0.0 checks above.
                None => continue,
                Some(sv) => sv,
            };
            // Clone to avoid manipulating the comparator's semver object.
            let mut compver = SemVer::new(&semver.version, Options::EMPTY)?;

            match comparator.operator.as_str() {
                ">" => {
                    if compver.prerelease.is_empty() {
                        compver.patch += 1;
                    } else {
                        compver.prerelease.push(Identifier::Numeric(0));
                    }
                    compver.raw = compver.format();
                    take_max(&mut set_min, compver);
                }
                "" | ">=" => {
                    take_max(&mut set_min, compver);
                }
                "<" | "<=" => {
                    // Ignore maximum versions
                }
                other => {
                    return Err(SemverError::UnexpectedOperation(other.to_string()));
                }
            }
        }

        if let Some(sm) = set_min {
            let replace = match &minver {
                None => true,
                Some(mv) => mv.compare(&sm) == Ordering::Greater,
            };
            if replace {
                minver = Some(sm);
            }
        }
    }

    match minver {
        Some(mv) if range.test_semver(&mv) => Ok(Some(mv)),
        _ => Ok(None),
    }
}

fn take_max(slot: &mut Option<SemVer>, candidate: SemVer) {
    let replace = match slot {
        None => true,
        Some(cur) => candidate.compare(cur) == Ordering::Greater,
    };
    if replace {
        *slot = Some(candidate);
    }
}

/// `validRange(range, options)` — `'*'` instead of `''` so truthiness works.
pub fn valid_range<R: ToRange + ?Sized>(range: &R, options: Options) -> Option<String> {
    let r = range.to_range(options).ok()?;
    let formatted = r.range();
    if formatted.is_empty() {
        Some("*".to_string())
    } else {
        Some(formatted.to_string())
    }
}

/// `outside(version, range, hilo, options)`
// The two low-bound branches below share a body but test different predicates;
// they are kept apart to stay line-for-line with the JavaScript source.
#[allow(clippy::if_same_then_else)]
pub fn outside(version: &str, range: &str, hilo: &str, options: Options) -> Result<bool> {
    let version = SemVer::new(version, options)?;
    let range = Range::new(range, options)?;

    // From now on, variable terms are as if we're in "gtr" mode, but note that
    // everything is flipped for the "ltr" function.
    type Cmp2 = fn(&SemVer, &SemVer, Options) -> Result<bool>;
    let (gtfn, ltefn, ltfn, comp, ecomp): (Cmp2, Cmp2, Cmp2, &str, &str) = match hilo {
        ">" => (gt, lte, lt, ">", ">="),
        "<" => (lt, gte, gt, "<", "<="),
        _ => return Err(SemverError::InvalidHilo),
    };

    // If it satisfies the range it is not outside
    if range.test_semver(&version) {
        return Ok(false);
    }

    let any_comparator = Comparator::new(">=0.0.0", Options::EMPTY)?;

    for comparators in &range.set {
        let mut high: Option<&Comparator> = None;
        let mut low: Option<&Comparator> = None;

        for comparator in comparators {
            let comparator = if comparator.is_any() {
                &any_comparator
            } else {
                comparator
            };
            if high.is_none() {
                high = Some(comparator);
            }
            if low.is_none() {
                low = Some(comparator);
            }

            let csv = comparator.semver.as_ref().expect("ANY was replaced");
            let hsv = high.unwrap().semver.as_ref().expect("ANY was replaced");
            let lsv = low.unwrap().semver.as_ref().expect("ANY was replaced");

            if gtfn(csv, hsv, options)? {
                high = Some(comparator);
            } else if ltfn(csv, lsv, options)? {
                low = Some(comparator);
            }
        }

        let high = match high {
            Some(h) => h,
            None => continue,
        };
        let low = low.expect("low set alongside high");

        // If the edge version comparator has an operator then our version
        // isn't outside it.
        if high.operator == comp || high.operator == ecomp {
            return Ok(false);
        }

        // If the lowest version comparator has an operator and our version is
        // less than it then it isn't higher than the range.
        let lsv = low.semver.as_ref().expect("ANY was replaced");
        if (low.operator.is_empty() || low.operator == comp)
            && ltefn(&version, lsv, Options::EMPTY)?
        {
            return Ok(false);
        } else if low.operator == ecomp && ltfn(&version, lsv, Options::EMPTY)? {
            return Ok(false);
        }
    }

    Ok(true)
}

/// `gtr(version, range, options)` — is the version greater than every version
/// the range allows?
pub fn gtr(version: &str, range: &str, options: Options) -> Result<bool> {
    outside(version, range, ">", options)
}

/// `ltr(version, range, options)`
pub fn ltr(version: &str, range: &str, options: Options) -> Result<bool> {
    outside(version, range, "<", options)
}

/// `intersects(r1, r2, options)`
pub fn intersects<A: ToRange + ?Sized, B: ToRange + ?Sized>(
    r1: &A,
    r2: &B,
    options: Options,
) -> Result<bool> {
    let r1 = r1.to_range(options)?;
    let r2 = r2.to_range(options)?;
    r1.intersects(&r2, options)
}

/// `simplifyRange(versions, range, options)`
///
/// Given a set of versions and a range, create a "simplified" range that
/// includes the same versions. If the original range is shorter, return it.
pub fn simplify(versions: &[String], range: &str, options: Options) -> Result<String> {
    // Note: no `new Range(...)` up front. JS goes through `satisfies`, which
    // swallows range errors, so an invalid range simply matches nothing.
    let range_obj = Range::new(range, options).ok();

    // `versions.sort((a, b) => compare(a, b, options))`
    let v: Vec<String> = if versions.len() < 2 {
        versions.to_vec()
    } else {
        let mut parsed = crate::functions::parse_for_sort(versions.to_vec(), options, false)?;
        parsed.sort_by(|a, b| a.0.compare(&b.0));
        parsed.into_iter().map(|(_, s)| s).collect()
    };

    let mut set: Vec<(String, Option<String>)> = Vec::new();
    let mut first: Option<String> = None;
    let mut prev: Option<String> = None;

    for version in &v {
        let included = range_obj.as_ref().map(|r| r.test(version)).unwrap_or(false);
        if included {
            prev = Some(version.clone());
            if first.is_none() {
                first = Some(version.clone());
            }
        } else {
            if let (Some(f), Some(p)) = (&first, &prev) {
                set.push((f.clone(), Some(p.clone())));
            }
            prev = None;
            first = None;
        }
    }
    if let Some(f) = &first {
        set.push((f.clone(), None));
    }

    let mut ranges: Vec<String> = Vec::new();
    for (min, max) in &set {
        match max {
            Some(max) if min == max => ranges.push(min.clone()),
            None if Some(min) == v.first() => ranges.push("*".to_string()),
            None => ranges.push(format!(">={min}")),
            Some(max) if Some(min) == v.first() => ranges.push(format!("<={max}")),
            Some(max) => ranges.push(format!("{min} - {max}")),
        }
    }

    let simplified = ranges.join(" || ");
    if simplified.len() < range.len() {
        Ok(simplified)
    } else {
        Ok(range.to_string())
    }
}

/// `subset(sub, dom, options)`
///
/// Complex range `r1 || r2 || ...` is a subset of `R1 || R2 || ...` iff every
/// simple range is a null set, or every non-null simple range is a subset of
/// some `R`.
pub fn subset(sub: &str, dom: &str, options: Options) -> Result<bool> {
    if sub == dom {
        return Ok(true);
    }

    let sub = Range::new(sub, options)?;
    let dom = Range::new(dom, options)?;
    let mut saw_non_null = false;

    'outer: for simple_sub in &sub.set {
        for simple_dom in &dom.set {
            let is_sub = simple_subset(simple_sub, simple_dom, options)?;
            saw_non_null = saw_non_null || is_sub.is_some();
            if is_sub == Some(true) {
                continue 'outer;
            }
        }
        // The null set is a subset of everything, but null simple ranges in a
        // complex range should be ignored.
        if saw_non_null {
            return Ok(false);
        }
    }

    Ok(true)
}

/// `None` is the JS `null` return ("this simple range is a null set").
fn simple_subset(
    sub: &[Comparator],
    dom: &[Comparator],
    options: Options,
) -> Result<Option<bool>> {
    let minimum_version_with_prerelease = vec![Comparator::new(">=0.0.0-0", Options::EMPTY)?];
    let minimum_version = vec![Comparator::new(">=0.0.0", Options::EMPTY)?];

    let mut sub: &[Comparator] = sub;
    let mut dom: &[Comparator] = dom;

    if sub.len() == 1 && sub[0].is_any() {
        if dom.len() == 1 && dom[0].is_any() {
            return Ok(Some(true));
        } else if options.include_prerelease {
            sub = &minimum_version_with_prerelease;
        } else {
            sub = &minimum_version;
        }
    }

    if dom.len() == 1 && dom[0].is_any() {
        if options.include_prerelease {
            return Ok(Some(true));
        } else {
            dom = &minimum_version;
        }
    }

    let mut eq_set: Vec<&SemVer> = Vec::new();
    let mut gt_c: Option<&Comparator> = None;
    let mut lt_c: Option<&Comparator> = None;

    for c in sub {
        if c.operator == ">" || c.operator == ">=" {
            gt_c = Some(higher_gt(gt_c, c, options)?);
        } else if c.operator == "<" || c.operator == "<=" {
            lt_c = Some(lower_lt(lt_c, c, options)?);
        } else if let Some(sv) = &c.semver {
            eq_set.push(sv);
        }
    }

    if eq_set.len() > 1 {
        return Ok(None);
    }

    let mut gtlt_comp: Option<Ordering> = None;
    if let (Some(g), Some(l)) = (gt_c, lt_c) {
        let c = compare(semver_of(g), semver_of(l), options)?;
        gtlt_comp = Some(c);
        if c == Ordering::Greater {
            return Ok(None);
        }
        if c == Ordering::Equal && (g.operator != ">=" || l.operator != "<=") {
            return Ok(None);
        }
    }

    // The JS source loops over `eqSet` here, but it has at most one member.
    if let Some(eq) = eq_set.first() {
        if let Some(g) = gt_c {
            if !satisfies_semver(eq, g.value.as_str(), options) {
                return Ok(None);
            }
        }
        if let Some(l) = lt_c {
            if !satisfies_semver(eq, l.value.as_str(), options) {
                return Ok(None);
            }
        }
        for c in dom {
            if !satisfies_semver(eq, c.value.as_str(), options) {
                return Ok(Some(false));
            }
        }
        return Ok(Some(true));
    }

    let mut has_dom_lt = false;
    let mut has_dom_gt = false;

    // If the subset has a prerelease, we need a comparator in the superset with
    // the same tuple and a prerelease, or it's not a subset.
    let mut need_dom_lt_pre: Option<&SemVer> = lt_c.and_then(|l| {
        let sv = semver_of(l);
        if !options.include_prerelease && !sv.prerelease.is_empty() {
            Some(sv)
        } else {
            None
        }
    });
    let mut need_dom_gt_pre: Option<&SemVer> = gt_c.and_then(|g| {
        let sv = semver_of(g);
        if !options.include_prerelease && !sv.prerelease.is_empty() {
            Some(sv)
        } else {
            None
        }
    });

    // exception: <1.2.3-0 is the same as <1.2.3
    if let (Some(need), Some(l)) = (need_dom_lt_pre, lt_c) {
        if need.prerelease.len() == 1
            && l.operator == "<"
            && need.prerelease[0] == Identifier::Numeric(0)
        {
            need_dom_lt_pre = None;
        }
    }

    for c in dom {
        has_dom_gt = has_dom_gt || c.operator == ">" || c.operator == ">=";
        has_dom_lt = has_dom_lt || c.operator == "<" || c.operator == "<=";

        if let Some(g) = gt_c {
            if let Some(need) = need_dom_gt_pre {
                if let Some(csv) = &c.semver {
                    if !csv.prerelease.is_empty()
                        && csv.major == need.major
                        && csv.minor == need.minor
                        && csv.patch == need.patch
                    {
                        need_dom_gt_pre = None;
                    }
                }
            }
            if c.operator == ">" || c.operator == ">=" {
                let higher = higher_gt(Some(g), c, options)?;
                if ptr::eq(higher, c) && !ptr::eq(higher, g) {
                    return Ok(Some(false));
                }
            } else if g.operator == ">=" && !c.test_semver(semver_of(g)) {
                return Ok(Some(false));
            }
        }

        if let Some(l) = lt_c {
            if let Some(need) = need_dom_lt_pre {
                if let Some(csv) = &c.semver {
                    if !csv.prerelease.is_empty()
                        && csv.major == need.major
                        && csv.minor == need.minor
                        && csv.patch == need.patch
                    {
                        need_dom_lt_pre = None;
                    }
                }
            }
            if c.operator == "<" || c.operator == "<=" {
                let lower = lower_lt(Some(l), c, options)?;
                if ptr::eq(lower, c) && !ptr::eq(lower, l) {
                    return Ok(Some(false));
                }
            } else if l.operator == "<=" && !c.test_semver(semver_of(l)) {
                return Ok(Some(false));
            }
        }

        if c.operator.is_empty()
            && (lt_c.is_some() || gt_c.is_some())
            && gtlt_comp != Some(Ordering::Equal)
        {
            return Ok(Some(false));
        }
    }

    // If there was a < or >, and nothing in the dom, then it must be false,
    // UNLESS it was limited by another range in the other direction.
    if gt_c.is_some() && has_dom_lt && lt_c.is_none() && gtlt_comp != Some(Ordering::Equal) {
        return Ok(Some(false));
    }
    if lt_c.is_some() && has_dom_gt && gt_c.is_none() && gtlt_comp != Some(Ordering::Equal) {
        return Ok(Some(false));
    }

    // We needed a prerelease range in a specific tuple, but didn't get one.
    if need_dom_gt_pre.is_some() || need_dom_lt_pre.is_some() {
        return Ok(Some(false));
    }

    Ok(Some(true))
}

fn semver_of(c: &Comparator) -> &SemVer {
    c.semver
        .as_ref()
        .expect("comparator with an operator always has a semver")
}

/// `>=1.2.3` is lower than `>1.2.3`
fn higher_gt<'a>(
    a: Option<&'a Comparator>,
    b: &'a Comparator,
    options: Options,
) -> Result<&'a Comparator> {
    let a = match a {
        None => return Ok(b),
        Some(a) => a,
    };
    let comp = compare(semver_of(a), semver_of(b), options)?;
    Ok(match comp {
        Ordering::Greater => a,
        Ordering::Less => b,
        Ordering::Equal => {
            if b.operator == ">" && a.operator == ">=" {
                b
            } else {
                a
            }
        }
    })
}

/// `<=1.2.3` is higher than `<1.2.3`
fn lower_lt<'a>(
    a: Option<&'a Comparator>,
    b: &'a Comparator,
    options: Options,
) -> Result<&'a Comparator> {
    let a = match a {
        None => return Ok(b),
        Some(a) => a,
    };
    let comp = compare(semver_of(a), semver_of(b), options)?;
    Ok(match comp {
        Ordering::Less => a,
        Ordering::Greater => b,
        Ordering::Equal => {
            if b.operator == "<" && a.operator == "<=" {
                b
            } else {
                a
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn satisfying() {
        let versions = strings(&["1.2.3", "1.2.4", "1.3.0", "2.0.0"]);
        assert_eq!(
            max_satisfying(&versions, "^1.2.3", Options::EMPTY).as_deref(),
            Some("1.3.0")
        );
        assert_eq!(
            min_satisfying(&versions, "^1.2.3", Options::EMPTY).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(max_satisfying(&versions, "^5", Options::EMPTY), None);
    }

    #[test]
    fn min_version_basics() {
        let mv = min_version("^1.2.3", Options::EMPTY).unwrap().unwrap();
        assert_eq!(mv.version, "1.2.3");
        let mv = min_version(">1.0.0", Options::EMPTY).unwrap().unwrap();
        assert_eq!(mv.version, "1.0.1");
        assert_eq!(min_version("*", Options::EMPTY).unwrap().unwrap().version, "0.0.0");
    }

    #[test]
    fn valid_range_normalizes() {
        assert_eq!(
            valid_range("1.2.3 - 2.0.0", Options::EMPTY).as_deref(),
            Some(">=1.2.3 <=2.0.0")
        );
        assert_eq!(valid_range("*", Options::EMPTY).as_deref(), Some("*"));
        assert_eq!(valid_range("not a range", Options::EMPTY), None);
    }

    #[test]
    fn gtr_ltr() {
        assert!(gtr("2.0.0", "^1.0.0", Options::EMPTY).unwrap());
        assert!(!gtr("1.5.0", "^1.0.0", Options::EMPTY).unwrap());
        assert!(ltr("0.9.0", "^1.0.0", Options::EMPTY).unwrap());
    }

    #[test]
    fn subset_basics() {
        assert!(subset("^1.2.3", ">=1.0.0", Options::EMPTY).unwrap());
        assert!(!subset(">=1.0.0", "^1.2.3", Options::EMPTY).unwrap());
        assert!(subset("1.2.3", "^1.2.0", Options::EMPTY).unwrap());
    }

    #[test]
    fn intersects_basics() {
        assert!(intersects("^1.2.3", ">=1.0.0", Options::EMPTY).unwrap());
        assert!(!intersects("^1.2.3", "^2.0.0", Options::EMPTY).unwrap());
    }

    #[test]
    fn to_comparators_shape() {
        assert_eq!(
            to_comparators("1.2.3 - 2.0.0", Options::EMPTY).unwrap(),
            vec![vec![">=1.2.3".to_string(), "<=2.0.0".to_string()]]
        );
    }

    #[test]
    fn simplify_basics() {
        let versions = strings(&["1.0.0", "1.1.0", "1.2.0", "1.3.0"]);
        let out = simplify(&versions, ">=1.0.0 <=1.3.0", Options::EMPTY).unwrap();
        assert_eq!(out, "*");
    }

    #[test]
    fn simplify_tolerates_an_invalid_range() {
        // JS routes through `satisfies`, which swallows range errors, so an
        // unparseable range simply matches nothing and simplifies to "".
        let versions = strings(&["1.0.0", "2.0.0"]);
        assert_eq!(simplify(&versions, "^00", Options::EMPTY).unwrap(), "");
        assert_eq!(simplify(&versions, "=.99.007", Options::EMPTY).unwrap(), "");
    }

    #[test]
    fn simplify_keeps_the_original_when_it_is_shorter() {
        let versions = strings(&["1.0.0", "1.1.0", "2.0.0"]);
        // ">=1.0.0 <=1.1.0" is longer than the input range, so the input wins.
        assert_eq!(simplify(&versions, "^1", Options::EMPTY).unwrap(), "^1");
    }

    #[test]
    fn subset_prerelease_tuples() {
        let o = Options::EMPTY;
        // >=1.2.3-pre is not a subset of >=1.0.0: it admits prereleases in the
        // 1.2.3 tuple that the superset does not.
        assert!(!subset(">=1.2.3-pre", ">=1.0.0", o).unwrap());
        assert!(subset(">=1.2.3-pre", ">=1.2.3-pre", o).unwrap());
        // <1.2.3-0 is the same as <1.2.3, so the tuple requirement is waived.
        assert!(subset("<1.2.3-0", "<2.0.0", o).unwrap());
        // A lone `<0.0.0-0` is still evaluated as a real comparator pair, so it
        // is *not* a subset; only a null branch inside a `||` gets ignored.
        assert!(!subset("<0.0.0-0", "^1.2.3", o).unwrap());
        assert!(subset("^1.2.3 || <0.0.0-0", "^1.2.3", o).unwrap());
        // With includePrerelease, * covers everything.
        assert!(subset("^1.2.3", "*", Options::EMPTY.include_prerelease(true)).unwrap());
    }

    #[test]
    fn outside_requires_a_valid_hilo() {
        let err = outside("1.2.3", "^1.0.0", "!", Options::EMPTY).unwrap_err();
        assert_eq!(err.to_string(), "Must provide a hilo val of \"<\" or \">\"");
    }

    #[test]
    fn min_version_edge_cases() {
        // A range no version can satisfy has no minimum.
        assert_eq!(min_version("<0.0.0-0", Options::EMPTY).unwrap(), None);
        // `>` bumps the patch, or appends a 0 to an existing prerelease.
        assert_eq!(
            min_version(">1.2.3-alpha", Options::EMPTY).unwrap().unwrap().version,
            "1.2.3-alpha.0"
        );
        // Alternatives take the lowest of the branch minimums.
        assert_eq!(
            min_version("^3.0.0 || ^1.2.3", Options::EMPTY).unwrap().unwrap().version,
            "1.2.3"
        );
    }

    #[test]
    fn huge_majors_follow_js_float_stringification() {
        // `+M + 1` happens in doubles in JS, so a 25-digit bound stringifies as
        // "1e+25" and produces an unparseable comparator rather than a big int.
        let err = valid_range("<=9999999999999999999999999", Options::EMPTY);
        assert_eq!(err, None);
        let err = Range::new("<=9999999999999999999999999", Options::EMPTY).unwrap_err();
        assert_eq!(err.to_string(), "Invalid comparator: <1e+25.0.0-0");
    }
}
