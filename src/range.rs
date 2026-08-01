//! Port of `classes/range.js`, including the caret/tilde/x-range/star/hyphen
//! desugaring helpers that live alongside it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use once_cell::sync::OnceCell;
use regex::{Captures, Regex};

use crate::comparator::Comparator;
use crate::error::{Result, SemverError};
use crate::options::Options;
use crate::re::{
    re, safe_re, t, CARET_TRIM_REPLACE, COMPARATOR_TRIM_REPLACE, SPACE_CHARACTERS,
    TILDE_TRIM_REPLACE,
};
use crate::semver::{js_number, js_number_to_string, SemVer};

/// Anything that can be turned into a [`Range`].
pub trait ToRange {
    fn to_range(&self, options: Options) -> Result<Range>;
}

impl ToRange for str {
    fn to_range(&self, options: Options) -> Result<Range> {
        Range::new(self, options)
    }
}

impl ToRange for String {
    fn to_range(&self, options: Options) -> Result<Range> {
        Range::new(self, options)
    }
}

impl ToRange for Range {
    fn to_range(&self, options: Options) -> Result<Range> {
        if self.loose == options.loose && self.include_prerelease == options.include_prerelease {
            Ok(self.clone())
        } else {
            Range::new(&self.raw, options)
        }
    }
}

impl<T: ToRange + ?Sized> ToRange for &T {
    fn to_range(&self, options: Options) -> Result<Range> {
        (**self).to_range(options)
    }
}

#[derive(Debug, Clone)]
pub struct Range {
    pub raw: String,
    pub options: Options,
    pub loose: bool,
    pub include_prerelease: bool,
    pub set: Vec<Vec<Comparator>>,
    formatted: OnceCell<String>,
}

impl Range {
    pub fn new(range: &str, options: Options) -> Result<Range> {
        // First reduce all whitespace as much as possible so we do not have to
        // rely on potentially slow regexes like `\s*`.
        let raw = SPACE_CHARACTERS.replace_all(range.trim(), " ").into_owned();

        let mut set: Vec<Vec<Comparator>> = Vec::new();
        for part in raw.split("||") {
            let comparators = parse_range(part.trim(), options)?;
            // Throw out any comparator lists that are empty; this generally
            // means it was not a valid range, which is allowed in loose mode.
            if !comparators.is_empty() {
                set.push(comparators);
            }
        }

        if set.is_empty() {
            return Err(SemverError::InvalidRange(raw));
        }

        // If we have any that are not the null set, throw out null sets.
        if set.len() > 1 {
            let first = set[0].clone();
            set.retain(|c| !c[0].is_null_set());
            if set.is_empty() {
                set = vec![first];
            } else if set.len() > 1 {
                // if we have any that are *, then the range is just *
                let any_index = set.iter().position(|c| c.len() == 1 && c[0].is_any());
                if let Some(i) = any_index {
                    let only = set.swap_remove(i);
                    set = vec![only];
                }
            }
        }

        Ok(Range {
            raw,
            options,
            loose: options.loose,
            include_prerelease: options.include_prerelease,
            set,
            formatted: OnceCell::new(),
        })
    }

    /// `new Range(comparatorInstance)`
    pub fn from_comparator(comp: &Comparator) -> Range {
        Range {
            raw: comp.value.clone(),
            options: comp.options,
            loose: comp.options.loose,
            include_prerelease: comp.options.include_prerelease,
            set: vec![vec![comp.clone()]],
            formatted: OnceCell::new(),
        }
    }

    /// The `range` getter: the canonical, re-serialized form.
    pub fn range(&self) -> &str {
        self.formatted.get_or_init(|| {
            let mut formatted = String::new();
            for (i, comps) in self.set.iter().enumerate() {
                if i > 0 {
                    formatted.push_str("||");
                }
                for (k, comp) in comps.iter().enumerate() {
                    if k > 0 {
                        formatted.push(' ');
                    }
                    formatted.push_str(comp.to_string().trim());
                }
            }
            formatted
        })
    }

    pub fn format(&self) -> &str {
        self.range()
    }

    /// `Range#test` — an unparseable version is simply `false`.
    pub fn test(&self, version: &str) -> bool {
        if version.is_empty() {
            return false;
        }
        match SemVer::new(version, self.options) {
            Ok(v) => self.test_semver(&v),
            Err(_) => false,
        }
    }

    pub fn test_semver(&self, version: &SemVer) -> bool {
        self.set
            .iter()
            .any(|set| test_set(set, version, self.options))
    }

    /// `Range#intersects`
    pub fn intersects(&self, range: &Range, options: Options) -> Result<bool> {
        for this_comparators in &self.set {
            if !is_satisfiable(this_comparators, options)? {
                continue;
            }
            for range_comparators in &range.set {
                if !is_satisfiable(range_comparators, options)? {
                    continue;
                }
                let mut every = true;
                'outer: for tc in this_comparators {
                    for rc in range_comparators {
                        if !tc.intersects(rc, options)? {
                            every = false;
                            break 'outer;
                        }
                    }
                }
                if every {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.range())
    }
}

// ---------------------------------------------------------------------------
// parseRange and its memoization
// ---------------------------------------------------------------------------

thread_local! {
    static RANGE_CACHE: RefCell<HashMap<(u8, String), Vec<Comparator>>> =
        RefCell::new(HashMap::new());
}

const RANGE_CACHE_MAX: usize = 1000;

/// `Range#parseRange`
pub fn parse_range(range: &str, options: Options) -> Result<Vec<Comparator>> {
    // strip build metadata so it can't bleed into the version
    let range = re(t::BUILD).replace_all(range, "").into_owned();

    let memo_key = (options.memo_flags(), range.clone());
    if let Some(cached) = RANGE_CACHE.with(|c| c.borrow().get(&memo_key).cloned()) {
        return Ok(cached);
    }

    let loose = options.loose;

    // `1.2.3 - 1.2.4` => `>=1.2.3 <=1.2.4`
    let hr = if loose {
        safe_re(t::HYPHENRANGELOOSE)
    } else {
        safe_re(t::HYPHENRANGE)
    };
    let range = hyphen_replace(&range, hr, options.include_prerelease);

    // `> 1.2.3 < 1.2.5` => `>1.2.3 <1.2.5`
    let range = safe_re(t::COMPARATORTRIM)
        .replace_all(&range, COMPARATOR_TRIM_REPLACE)
        .into_owned();

    // `~ 1.2.3` => `~1.2.3`
    let range = safe_re(t::TILDETRIM)
        .replace_all(&range, TILDE_TRIM_REPLACE)
        .into_owned();

    // `^ 1.2.3` => `^1.2.3`
    let range = safe_re(t::CARETTRIM)
        .replace_all(&range, CARET_TRIM_REPLACE)
        .into_owned();

    // At this point, the range is completely trimmed and ready to be split
    // into comparators.
    let joined: Vec<String> = range
        .split(' ')
        .map(|comp| parse_comparator(comp, options))
        .collect();
    let joined = joined.join(" ");

    let mut range_list: Vec<String> = SPACE_CHARACTERS
        .split(&joined)
        // >=0.0.0 is equivalent to *
        .map(|comp| replace_gte0(comp, options))
        .collect();

    if loose {
        // in loose mode, throw out any that are not valid comparators
        let comparator_loose = safe_re(t::COMPARATORLOOSE);
        range_list.retain(|comp| comparator_loose.is_match(comp));
    }

    // if any comparators are the null set, then replace with JUST null set;
    // if more than one comparator, remove any * comparators;
    // also, don't include the same comparator more than once.
    // Every comparator is constructed up front (so an invalid one still throws)
    // before the null-set short circuit runs.
    let mut comparators = Vec::with_capacity(range_list.len());
    for comp in &range_list {
        comparators.push(Comparator::new(comp, options)?);
    }

    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Comparator> = HashMap::new();
    let mut null_set: Option<Comparator> = None;

    for comparator in comparators {
        if comparator.is_null_set() {
            null_set = Some(comparator);
            break;
        }
        if !map.contains_key(&comparator.value) {
            order.push(comparator.value.clone());
        }
        map.insert(comparator.value.clone(), comparator);
    }

    let result = if let Some(ns) = null_set {
        vec![ns]
    } else {
        if map.len() > 1 && map.contains_key("") {
            map.remove("");
            order.retain(|k| !k.is_empty());
        }
        order
            .into_iter()
            .filter_map(|k| map.remove(&k))
            .collect::<Vec<_>>()
    };

    RANGE_CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() >= RANGE_CACHE_MAX {
            cache.clear();
        }
        cache.insert(memo_key, result.clone());
    });

    Ok(result)
}

/// `isSatisfiable` — is there a version that can satisfy every comparator?
fn is_satisfiable(comparators: &[Comparator], options: Options) -> Result<bool> {
    let mut remaining: Vec<&Comparator> = comparators.iter().collect();
    let mut test_comparator = remaining.pop();
    let mut result = true;

    while result && !remaining.is_empty() {
        let tc = match test_comparator {
            Some(tc) => tc,
            None => break,
        };
        let mut every = true;
        for other in &remaining {
            if !tc.intersects(other, options)? {
                every = false;
                break;
            }
        }
        result = every;
        test_comparator = remaining.pop();
    }

    Ok(result)
}

/// `parseComparator` — comprised of xranges, tildes, stars, and gtlt's.
fn parse_comparator(comp: &str, options: Options) -> String {
    // note: non-global replace, only the first build metadata run is removed
    let comp = safe_re(t::BUILD).replace(comp, "").into_owned();
    let comp = replace_carets(&comp, options);
    let comp = replace_tildes(&comp, options);
    let comp = replace_xranges(&comp, options);
    replace_stars(&comp, options)
}

/// `isX`
fn is_x(id: Option<&str>) -> bool {
    match id {
        None => true,
        Some(s) => s.is_empty() || s.eq_ignore_ascii_case("x") || s == "*",
    }
}

/// `invalidXRangeOrder`
fn invalid_xrange_order(m: Option<&str>, mi: Option<&str>, p: Option<&str>) -> bool {
    (is_x(m) && !is_x(mi))
        || (is_x(mi) && p.map(|s| !s.is_empty()).unwrap_or(false) && !is_x(p))
}

/// `` `${+value + 1}` ``
///
/// Deliberately routed through `f64` rather than an integer type: JS does the
/// arithmetic in doubles, so values past `2**53` lose precision and values past
/// `1e21` stringify in exponential form. Both quirks leak into the generated
/// comparator text and therefore into error messages.
fn plus_one(s: &str) -> String {
    let n = match s.parse::<u64>() {
        // Fast, exact path for everything a sane range contains.
        Ok(n) if n < (1u64 << 53) => return (n + 1).to_string(),
        _ => js_number(s).unwrap_or(f64::NAN),
    };
    js_number_to_string(n + 1.0)
}

fn group<'a>(caps: &Captures<'a>, i: usize) -> Option<&'a str> {
    caps.get(i).map(|g| g.as_str())
}

/// `replaceTildes`
fn replace_tildes(comp: &str, options: Options) -> String {
    SPACE_CHARACTERS
        .split(comp.trim())
        .map(|c| replace_tilde(c, options))
        .collect::<Vec<_>>()
        .join(" ")
}

/// ```text
/// ~, ~>        --> * (any, kinda silly)
/// ~2, ~2.x     --> >=2.0.0 <3.0.0-0
/// ~1.2, ~1.2.x --> >=1.2.0 <1.3.0-0
/// ~1.2.3       --> >=1.2.3 <1.3.0-0
/// ~0.0.1       --> >=0.0.1 <0.1.0-0
/// ```
fn replace_tilde(comp: &str, options: Options) -> String {
    let r = if options.loose {
        safe_re(t::TILDELOOSE)
    } else {
        safe_re(t::TILDE)
    };
    // The pattern is anchored, so at most the whole string is replaced.
    let caps = match r.captures(comp) {
        Some(c) => c,
        None => return comp.to_string(),
    };

    // If we're including prereleases in the match, then the lower bound is
    // `-0`, the lowest possible prerelease value.
    let z = if options.include_prerelease { "-0" } else { "" };

    let major = group(&caps, 1);
    let minor = group(&caps, 2);
    let patch = group(&caps, 3);
    let pr = group(&caps, 4).filter(|s| !s.is_empty());

    if is_x(major) {
        String::new()
    } else if is_x(minor) {
        let m = major.unwrap();
        format!(">={m}.0.0{z} <{}.0.0-0", plus_one(m))
    } else if is_x(patch) {
        let (m, mi) = (major.unwrap(), minor.unwrap());
        format!(">={m}.{mi}.0{z} <{m}.{}.0-0", plus_one(mi))
    } else if let Some(pr) = pr {
        let (m, mi, p) = (major.unwrap(), minor.unwrap(), patch.unwrap());
        format!(">={m}.{mi}.{p}-{pr} <{m}.{}.0-0", plus_one(mi))
    } else {
        let (m, mi, p) = (major.unwrap(), minor.unwrap(), patch.unwrap());
        format!(">={m}.{mi}.{p} <{m}.{}.0-0", plus_one(mi))
    }
}

/// `replaceCarets`
fn replace_carets(comp: &str, options: Options) -> String {
    SPACE_CHARACTERS
        .split(comp.trim())
        .map(|c| replace_caret(c, options))
        .collect::<Vec<_>>()
        .join(" ")
}

/// ```text
/// ^, ^x         --> * (any, kinda silly)
/// ^2, ^2.x      --> >=2.0.0 <3.0.0-0
/// ^1.2, ^1.2.x  --> >=1.2.0 <2.0.0-0
/// ^1.2.3        --> >=1.2.3 <2.0.0-0
/// ^0.0.1        --> >=0.0.1 <0.0.2-0
/// ^0.1.0        --> >=0.1.0 <0.2.0-0
/// ```
fn replace_caret(comp: &str, options: Options) -> String {
    let r = if options.loose {
        safe_re(t::CARETLOOSE)
    } else {
        safe_re(t::CARET)
    };
    let caps = match r.captures(comp) {
        Some(c) => c,
        None => return comp.to_string(),
    };

    let z = if options.include_prerelease { "-0" } else { "" };

    let major = group(&caps, 1);
    let minor = group(&caps, 2);
    let patch = group(&caps, 3);
    let pr = group(&caps, 4).filter(|s| !s.is_empty());

    if is_x(major) {
        String::new()
    } else if is_x(minor) {
        let m = major.unwrap();
        format!(">={m}.0.0{z} <{}.0.0-0", plus_one(m))
    } else if is_x(patch) {
        let (m, mi) = (major.unwrap(), minor.unwrap());
        if m == "0" {
            format!(">={m}.{mi}.0{z} <{m}.{}.0-0", plus_one(mi))
        } else {
            format!(">={m}.{mi}.0{z} <{}.0.0-0", plus_one(m))
        }
    } else if let Some(pr) = pr {
        let (m, mi, p) = (major.unwrap(), minor.unwrap(), patch.unwrap());
        if m == "0" {
            if mi == "0" {
                format!(">={m}.{mi}.{p}-{pr} <{m}.{mi}.{}-0", plus_one(p))
            } else {
                format!(">={m}.{mi}.{p}-{pr} <{m}.{}.0-0", plus_one(mi))
            }
        } else {
            format!(">={m}.{mi}.{p}-{pr} <{}.0.0-0", plus_one(m))
        }
    } else {
        let (m, mi, p) = (major.unwrap(), minor.unwrap(), patch.unwrap());
        if m == "0" {
            if mi == "0" {
                format!(">={m}.{mi}.{p} <{m}.{mi}.{}-0", plus_one(p))
            } else {
                format!(">={m}.{mi}.{p} <{m}.{}.0-0", plus_one(mi))
            }
        } else {
            format!(">={m}.{mi}.{p} <{}.0.0-0", plus_one(m))
        }
    }
}

/// `replaceXRanges`
fn replace_xranges(comp: &str, options: Options) -> String {
    SPACE_CHARACTERS
        .split(comp)
        .map(|c| replace_xrange(c, options))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `replaceXRange`
fn replace_xrange(comp: &str, options: Options) -> String {
    let comp = comp.trim();
    let r = if options.loose {
        safe_re(t::XRANGELOOSE)
    } else {
        safe_re(t::XRANGE)
    };
    let caps = match r.captures(comp) {
        Some(c) => c,
        None => return comp.to_string(),
    };

    let full_match = caps.get(0).map(|g| g.as_str()).unwrap_or(comp).to_string();
    let mut gtlt = group(&caps, 1).unwrap_or("").to_string();
    let major = group(&caps, 2);
    let minor = group(&caps, 3);
    let patch = group(&caps, 4);

    if invalid_xrange_order(major, minor, patch) {
        return comp.to_string();
    }

    let x_major = is_x(major);
    let x_minor = x_major || is_x(minor);
    let x_patch = x_minor || is_x(patch);
    let any_x = x_patch;

    if gtlt == "=" && any_x {
        gtlt.clear();
    }

    // if we're including prereleases in the match, then we need to fix this to
    // `-0`, the lowest possible prerelease value
    let mut pr = if options.include_prerelease { "-0" } else { "" }.to_string();

    if x_major {
        if gtlt == ">" || gtlt == "<" {
            // nothing is allowed
            "<0.0.0-0".to_string()
        } else {
            // nothing is forbidden
            "*".to_string()
        }
    } else if !gtlt.is_empty() && any_x {
        let mut m = major.unwrap_or("").to_string();
        // we know patch is an x, because we have any x at all
        let mut mi = if x_minor {
            "0".to_string()
        } else {
            minor.unwrap_or("").to_string()
        };
        let mut p = "0".to_string();

        if gtlt == ">" {
            // >1 => >=2.0.0 ; >1.2 => >=1.3.0
            gtlt = ">=".to_string();
            if x_minor {
                m = plus_one(&m);
                mi = "0".to_string();
                p = "0".to_string();
            } else {
                mi = plus_one(&mi);
                p = "0".to_string();
            }
        } else if gtlt == "<=" {
            // <=0.7.x is actually <0.8.0, since any 0.7.x should pass
            gtlt = "<".to_string();
            if x_minor {
                m = plus_one(&m);
            } else {
                mi = plus_one(&mi);
            }
        }

        if gtlt == "<" {
            pr = "-0".to_string();
        }

        format!("{gtlt}{m}.{mi}.{p}{pr}")
    } else if x_minor {
        let m = major.unwrap();
        format!(">={m}.0.0{pr} <{}.0.0-0", plus_one(m))
    } else if x_patch {
        let (m, mi) = (major.unwrap(), minor.unwrap());
        format!(">={m}.{mi}.0{pr} <{m}.{}.0-0", plus_one(mi))
    } else {
        full_match
    }
}

/// `replaceStars` — because `*` is AND-ed with everything else, just remove it.
fn replace_stars(comp: &str, _options: Options) -> String {
    // Looseness is ignored here. Star is always as loose as it gets!
    safe_re(t::STAR).replace(comp.trim(), "").into_owned()
}

/// `replaceGTE0`
fn replace_gte0(comp: &str, options: Options) -> String {
    let token = if options.include_prerelease {
        t::GTE0PRE
    } else {
        t::GTE0
    };
    safe_re(token).replace(comp.trim(), "").into_owned()
}

/// `hyphenReplace`
///
/// ```text
/// 1.2 - 3.4.5 => >=1.2.0 <=3.4.5
/// 1.2.3 - 3.4 => >=1.2.0 <3.5.0-0
/// 1.2 - 3.4   => >=1.2.0 <3.5.0-0
/// ```
fn hyphen_replace(range: &str, r: &Regex, inc_pr: bool) -> String {
    let caps = match r.captures(range) {
        Some(c) => c,
        None => return range.to_string(),
    };

    let from_full = group(&caps, 1).unwrap_or("");
    let f_major = group(&caps, 2);
    let f_minor = group(&caps, 3);
    let f_patch = group(&caps, 4);
    let f_pr = group(&caps, 5).filter(|s| !s.is_empty());

    let to_full = group(&caps, 7).unwrap_or("");
    let t_major = group(&caps, 8);
    let t_minor = group(&caps, 9);
    let t_patch = group(&caps, 10);
    let t_pr = group(&caps, 11).filter(|s| !s.is_empty());

    let z = if inc_pr { "-0" } else { "" };

    let from = if is_x(f_major) {
        String::new()
    } else if is_x(f_minor) {
        format!(">={}.0.0{z}", f_major.unwrap())
    } else if is_x(f_patch) {
        format!(">={}.{}.0{z}", f_major.unwrap(), f_minor.unwrap())
    } else if f_pr.is_some() {
        format!(">={from_full}")
    } else {
        format!(">={from_full}{z}")
    };

    let to = if is_x(t_major) {
        String::new()
    } else if is_x(t_minor) {
        format!("<{}.0.0-0", plus_one(t_major.unwrap()))
    } else if is_x(t_patch) {
        format!("<{}.{}.0-0", t_major.unwrap(), plus_one(t_minor.unwrap()))
    } else if let Some(tpr) = t_pr {
        format!(
            "<={}.{}.{}-{}",
            t_major.unwrap(),
            t_minor.unwrap(),
            t_patch.unwrap(),
            tpr
        )
    } else if inc_pr {
        format!(
            "<{}.{}.{}-0",
            t_major.unwrap(),
            t_minor.unwrap(),
            plus_one(t_patch.unwrap())
        )
    } else {
        format!("<={to_full}")
    };

    format!("{from} {to}").trim().to_string()
}

/// `testSet`
fn test_set(set: &[Comparator], version: &SemVer, options: Options) -> bool {
    for comparator in set {
        if !comparator.test_semver(version) {
            return false;
        }
    }

    if !version.prerelease.is_empty() && !options.include_prerelease {
        // Find the set of versions that are allowed to have prereleases.
        // For example, ^1.2.3-pr.1 desugars to >=1.2.3-pr.1 <2.0.0, and that
        // should allow `1.2.3-pr.2` to pass, but not `1.2.4-alpha.notready`.
        for comparator in set {
            let allowed = match &comparator.semver {
                None => continue,
                Some(sv) => sv,
            };
            if !allowed.prerelease.is_empty()
                && allowed.major == version.major
                && allowed.minor == version.minor
                && allowed.patch == version.patch
            {
                return true;
            }
        }

        // Version has a -pre, but it's not one of the ones we like.
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> Range {
        Range::new(s, Options::EMPTY).unwrap()
    }

    #[test]
    fn caret_expansion() {
        assert_eq!(r("^1.2.3").range(), ">=1.2.3 <2.0.0-0");
        assert_eq!(r("^0.1.2").range(), ">=0.1.2 <0.2.0-0");
        assert_eq!(r("^0.0.3").range(), ">=0.0.3 <0.0.4-0");
        assert_eq!(r("^1.2.x").range(), ">=1.2.0 <2.0.0-0");
    }

    #[test]
    fn tilde_expansion() {
        assert_eq!(r("~1.2.3").range(), ">=1.2.3 <1.3.0-0");
        assert_eq!(r("~1.2").range(), ">=1.2.0 <1.3.0-0");
        assert_eq!(r("~1").range(), ">=1.0.0 <2.0.0-0");
    }

    #[test]
    fn hyphen_and_x_ranges() {
        assert_eq!(r("1.2.3 - 2.3.4").range(), ">=1.2.3 <=2.3.4");
        assert_eq!(r("1.2 - 2.3").range(), ">=1.2.0 <2.4.0-0");
        assert_eq!(r("1.2.x").range(), ">=1.2.0 <1.3.0-0");
        assert_eq!(r("*").range(), "");
    }

    #[test]
    fn prerelease_gating() {
        assert!(!r("^1.2.3").test("1.2.4-alpha"));
        assert!(r(">=1.2.3-pr.1").test("1.2.3-pr.2"));
        let ip = Range::new("^1.2.3", Options::EMPTY.include_prerelease(true)).unwrap();
        assert!(ip.test("1.2.4-alpha"));
    }

    #[test]
    fn invalid_range_message() {
        let err = Range::new("sadf||asdf", Options::LOOSE).unwrap_err();
        assert_eq!(err.to_string(), "Invalid SemVer Range: sadf||asdf");
    }
}
