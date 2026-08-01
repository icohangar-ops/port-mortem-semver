//! Port of `internal/re.js`.
//!
//! The tokens are created in exactly the same order as the JavaScript source so
//! that the numeric token indices (`semver.tokens`) line up, and every pattern
//! is assembled by interpolating previously created patterns just like
//! `createToken` does. That keeps the capture-group numbering identical, which
//! the rest of the port relies on.
//!
//! Both flavours are built: `src`/`re` are the literal patterns exported to
//! userland, and `safe_src`/`safe_re` are the bounded-repetition rewrites that
//! node-semver actually uses internally.

use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;

use crate::constants::{MAX_LENGTH, MAX_SAFE_BUILD_LENGTH, MAX_SAFE_COMPONENT_LENGTH};

const LETTERDASHNUMBER: &str = "[a-zA-Z0-9-]";

/// Token indices. These match `semver.tokens` from the JS package.
pub mod t {
    pub const NUMERICIDENTIFIER: usize = 0;
    pub const NUMERICIDENTIFIERLOOSE: usize = 1;
    pub const NONNUMERICIDENTIFIER: usize = 2;
    pub const MAINVERSION: usize = 3;
    pub const MAINVERSIONLOOSE: usize = 4;
    pub const PRERELEASEIDENTIFIER: usize = 5;
    pub const PRERELEASEIDENTIFIERLOOSE: usize = 6;
    pub const PRERELEASE: usize = 7;
    pub const PRERELEASELOOSE: usize = 8;
    pub const BUILDIDENTIFIER: usize = 9;
    pub const BUILD: usize = 10;
    pub const FULLPLAIN: usize = 11;
    pub const FULL: usize = 12;
    pub const LOOSEPLAIN: usize = 13;
    pub const LOOSE: usize = 14;
    pub const GTLT: usize = 15;
    pub const XRANGEIDENTIFIERLOOSE: usize = 16;
    pub const XRANGEIDENTIFIER: usize = 17;
    pub const XRANGEPLAIN: usize = 18;
    pub const XRANGEPLAINLOOSE: usize = 19;
    pub const XRANGE: usize = 20;
    pub const XRANGELOOSE: usize = 21;
    pub const COERCEPLAIN: usize = 22;
    pub const COERCE: usize = 23;
    pub const COERCEFULL: usize = 24;
    pub const COERCERTL: usize = 25;
    pub const COERCERTLFULL: usize = 26;
    pub const LONETILDE: usize = 27;
    pub const TILDETRIM: usize = 28;
    pub const TILDE: usize = 29;
    pub const TILDELOOSE: usize = 30;
    pub const LONECARET: usize = 31;
    pub const CARETTRIM: usize = 32;
    pub const CARET: usize = 33;
    pub const CARETLOOSE: usize = 34;
    pub const COMPARATORLOOSE: usize = 35;
    pub const COMPARATOR: usize = 36;
    pub const COMPARATORTRIM: usize = 37;
    pub const HYPHENRANGE: usize = 38;
    pub const HYPHENRANGELOOSE: usize = 39;
    pub const STAR: usize = 40;
    pub const GTE0: usize = 41;
    pub const GTE0PRE: usize = 42;

    pub const COUNT: usize = 43;
}

pub const TILDE_TRIM_REPLACE: &str = "$1~";
pub const CARET_TRIM_REPLACE: &str = "$1^";
pub const COMPARATOR_TRIM_REPLACE: &str = "$1$2$3";

/// `makeSafeRegex` — swap unbounded repetition for bounded repetition.
fn make_safe_regex(value: &str) -> String {
    let replacements: [(&str, usize); 3] = [
        ("\\s", 1),
        ("\\d", MAX_LENGTH),
        (LETTERDASHNUMBER, MAX_SAFE_BUILD_LENGTH),
    ];
    let mut value = value.to_string();
    for (token, max) in replacements {
        value = value
            .replace(&format!("{token}*"), &format!("{token}{{0,{max}}}"))
            .replace(&format!("{token}+"), &format!("{token}{{1,{max}}}"));
    }
    value
}

pub struct Tokens {
    pub src: Vec<String>,
    pub safe_src: Vec<String>,
    re: Vec<OnceCell<Regex>>,
    safe_re: Vec<OnceCell<Regex>>,
}

impl Tokens {
    /// The literal (userland) regex for a token, compiled on first use.
    pub fn re(&self, index: usize) -> &Regex {
        self.re[index].get_or_init(|| compile(&self.src[index]))
    }

    /// The ReDoS-hardened regex for a token, compiled on first use.
    pub fn safe_re(&self, index: usize) -> &Regex {
        self.safe_re[index].get_or_init(|| compile(&self.safe_src[index]))
    }
}

struct Builder {
    src: Vec<String>,
    safe_src: Vec<String>,
}

impl Builder {
    fn new() -> Self {
        Builder {
            src: Vec::with_capacity(t::COUNT),
            safe_src: Vec::with_capacity(t::COUNT),
        }
    }

    fn create(&mut self, value: String) {
        self.safe_src.push(make_safe_regex(&value));
        self.src.push(value);
    }

    fn s(&self, index: usize) -> &str {
        &self.src[index]
    }
}

/// JavaScript's `\d` (without the `u` flag) is exactly `[0-9]`, whereas Rust's
/// is the full Unicode decimal-number class. Spelling it out both matches the
/// original more closely and keeps the compiled program small: the "safe"
/// patterns repeat `\d` up to 256 times, and a Unicode class costs dozens of
/// ranges per repetition. `[^\d]` is the only occurrence inside a class.
fn ascii_digits(pattern: &str) -> String {
    pattern.replace("[^\\d]", "[^0-9]").replace("\\d", "[0-9]")
}

fn compile(pattern: &str) -> Regex {
    // Bounded repetitions in the "safe" patterns inflate the compiled program,
    // so the program size limit has to be well above the crate default.
    //
    // The lazy DFA budget goes the other way. Those same repetitions give the
    // hybrid engine an enormous state space to explore, and it will happily
    // spend a hundred milliseconds building it out. Versions and ranges are
    // short, so capping the cache low makes the engine fall back to the NFA
    // searchers, which is dramatically faster here and costs nothing on the
    // inputs semver actually sees.
    regex::RegexBuilder::new(&ascii_digits(pattern))
        .size_limit(64 * 1024 * 1024)
        .dfa_size_limit(256 * 1024)
        .build()
        .unwrap_or_else(|e| panic!("failed to compile semver regex {pattern:?}: {e}"))
}

static TOKENS: Lazy<Tokens> = Lazy::new(|| {
    let mut b = Builder::new();

    // ## Numeric Identifier
    b.create(r"0|[1-9]\d*".to_string());
    b.create(r"\d+".to_string());

    // ## Non-numeric Identifier
    b.create(format!(r"\d*[a-zA-Z-]{LETTERDASHNUMBER}*"));

    // ## Main Version
    b.create(format!(
        r"({0})\.({0})\.({0})",
        b.s(t::NUMERICIDENTIFIER)
    ));
    b.create(format!(
        r"({0})\.({0})\.({0})",
        b.s(t::NUMERICIDENTIFIERLOOSE)
    ));

    // ## Pre-release Version Identifier
    b.create(format!(
        "(?:{}|{})",
        b.s(t::NONNUMERICIDENTIFIER),
        b.s(t::NUMERICIDENTIFIER)
    ));
    b.create(format!(
        "(?:{}|{})",
        b.s(t::NONNUMERICIDENTIFIER),
        b.s(t::NUMERICIDENTIFIERLOOSE)
    ));

    // ## Pre-release Version
    b.create(format!(
        r"(?:-({0}(?:\.{0})*))",
        b.s(t::PRERELEASEIDENTIFIER)
    ));
    b.create(format!(
        r"(?:-?({0}(?:\.{0})*))",
        b.s(t::PRERELEASEIDENTIFIERLOOSE)
    ));

    // ## Build Metadata Identifier
    b.create(format!("{LETTERDASHNUMBER}+"));

    // ## Build Metadata
    b.create(format!(
        r"(?:\+({0}(?:\.{0})*))",
        b.s(t::BUILDIDENTIFIER)
    ));

    // ## Full Version String
    b.create(format!(
        "v?{}{}?{}?",
        b.s(t::MAINVERSION),
        b.s(t::PRERELEASE),
        b.s(t::BUILD)
    ));
    b.create(format!("^{}$", b.s(t::FULLPLAIN)));

    // like full, but allows v1.2.3 and =1.2.3, and 1.0.0alpha1.
    b.create(format!(
        r"[v=\s]*{}{}?{}?",
        b.s(t::MAINVERSIONLOOSE),
        b.s(t::PRERELEASELOOSE),
        b.s(t::BUILD)
    ));
    b.create(format!("^{}$", b.s(t::LOOSEPLAIN)));

    b.create("((?:<|>)?=?)".to_string());

    // Something like "2.*" or "1.2.x".
    b.create(format!(
        r"{}|x|X|\*",
        b.s(t::NUMERICIDENTIFIERLOOSE)
    ));
    b.create(format!(r"{}|x|X|\*", b.s(t::NUMERICIDENTIFIER)));

    b.create(format!(
        r"[v=\s]*({0})(?:\.({0})(?:\.({0})(?:{1})?{2}?)?)?",
        b.s(t::XRANGEIDENTIFIER),
        b.s(t::PRERELEASE),
        b.s(t::BUILD)
    ));
    b.create(format!(
        r"[v=\s]*({0})(?:\.({0})(?:\.({0})(?:{1})?{2}?)?)?",
        b.s(t::XRANGEIDENTIFIERLOOSE),
        b.s(t::PRERELEASELOOSE),
        b.s(t::BUILD)
    ));

    b.create(format!(
        r"^{}\s*{}$",
        b.s(t::GTLT),
        b.s(t::XRANGEPLAIN)
    ));
    b.create(format!(
        r"^{}\s*{}$",
        b.s(t::GTLT),
        b.s(t::XRANGEPLAINLOOSE)
    ));

    // Coercion.
    b.create(format!(
        r"(^|[^\d])(\d{{1,{0}}})(?:\.(\d{{1,{0}}}))?(?:\.(\d{{1,{0}}}))?",
        MAX_SAFE_COMPONENT_LENGTH
    ));
    b.create(format!(r"{}(?:$|[^\d])", b.s(t::COERCEPLAIN)));
    b.create(format!(
        r"{}(?:{})?(?:{})?(?:$|[^\d])",
        b.s(t::COERCEPLAIN),
        b.s(t::PRERELEASE),
        b.s(t::BUILD)
    ));
    b.create(b.s(t::COERCE).to_string());
    b.create(b.s(t::COERCEFULL).to_string());

    // Tilde ranges.
    b.create("(?:~>?)".to_string());
    b.create(format!(r"(\s*){}\s+", b.s(t::LONETILDE)));
    b.create(format!("^{}{}$", b.s(t::LONETILDE), b.s(t::XRANGEPLAIN)));
    b.create(format!(
        "^{}{}$",
        b.s(t::LONETILDE),
        b.s(t::XRANGEPLAINLOOSE)
    ));

    // Caret ranges.
    b.create(r"(?:\^)".to_string());
    b.create(format!(r"(\s*){}\s+", b.s(t::LONECARET)));
    b.create(format!("^{}{}$", b.s(t::LONECARET), b.s(t::XRANGEPLAIN)));
    b.create(format!(
        "^{}{}$",
        b.s(t::LONECARET),
        b.s(t::XRANGEPLAINLOOSE)
    ));

    // A simple gt/lt/eq thing, or just "" to indicate "any version"
    b.create(format!(
        r"^{}\s*({})$|^$",
        b.s(t::GTLT),
        b.s(t::LOOSEPLAIN)
    ));
    b.create(format!(
        r"^{}\s*({})$|^$",
        b.s(t::GTLT),
        b.s(t::FULLPLAIN)
    ));

    // Strip whitespace between the gtlt and the thing it modifies.
    b.create(format!(
        r"(\s*){}\s*({}|{})",
        b.s(t::GTLT),
        b.s(t::LOOSEPLAIN),
        b.s(t::XRANGEPLAIN)
    ));

    // Something like `1.2.3 - 1.2.4`
    b.create(format!(
        r"^\s*({0})\s+-\s+({0})\s*$",
        b.s(t::XRANGEPLAIN)
    ));
    b.create(format!(
        r"^\s*({0})\s+-\s+({0})\s*$",
        b.s(t::XRANGEPLAINLOOSE)
    ));

    // Star ranges basically just allow anything at all.
    b.create(r"(<|>)?=?\s*\*".to_string());
    // >=0.0.0 is like a star
    b.create(r"^\s*>=\s*0\.0\.0\s*$".to_string());
    b.create(r"^\s*>=\s*0\.0\.0-0\s*$".to_string());

    debug_assert_eq!(b.src.len(), t::COUNT);

    // The bounded-repetition rewrites compile into large programs, and a given
    // operation only ever touches a handful of them, so each pattern is
    // compiled on first use rather than all 86 up front.
    let re = b.src.iter().map(|_| OnceCell::new()).collect();
    let safe_re = b.safe_src.iter().map(|_| OnceCell::new()).collect();

    Tokens {
        src: b.src,
        safe_src: b.safe_src,
        re,
        safe_re,
    }
});

pub fn tokens() -> &'static Tokens {
    &TOKENS
}

/// The literal (userland) regex for a token.
pub fn re(index: usize) -> &'static Regex {
    TOKENS.re(index)
}

/// The ReDoS-hardened regex used internally by node-semver.
pub fn safe_re(index: usize) -> &'static Regex {
    TOKENS.safe_re(index)
}

pub fn src(index: usize) -> &'static str {
    &TOKENS.src[index]
}

pub fn safe_src(index: usize) -> &'static str {
    &TOKENS.safe_src[index]
}

/// `new RegExp(src[t.BUILD], 'g')` — the unbounded global build stripper used
/// by `Range#parseRange`.
pub static BUILD_STRIP_RE: Lazy<&'static Regex> = Lazy::new(|| re(t::BUILD));

/// `/\s+/g`
pub static SPACE_CHARACTERS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tokens_compile() {
        let tk = tokens();
        assert_eq!(tk.src.len(), t::COUNT);
        assert_eq!(tk.re.len(), t::COUNT);
        assert_eq!(tk.safe_re.len(), t::COUNT);
    }

    #[test]
    fn full_matches_capture_groups() {
        let caps = safe_re(t::FULL).captures("1.2.3-alpha.1+build.5").unwrap();
        assert_eq!(&caps[1], "1");
        assert_eq!(&caps[2], "2");
        assert_eq!(&caps[3], "3");
        assert_eq!(&caps[4], "alpha.1");
        assert_eq!(&caps[5], "build.5");
    }

    #[test]
    fn safe_rewrite() {
        assert_eq!(make_safe_regex(r"\s*"), r"\s{0,1}");
        assert_eq!(make_safe_regex(r"\d+"), r"\d{1,256}");
        assert_eq!(make_safe_regex("[a-zA-Z0-9-]+"), "[a-zA-Z0-9-]{1,250}");
    }
}
