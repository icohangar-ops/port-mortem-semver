//! Port of `bin/semver.js` — a standalone semver comparison program.
//!
//! Exits successfully and prints matching version(s) if any supplied version is
//! valid and passes all tests. Argument handling deliberately mirrors the
//! hand-rolled `switch` in the JavaScript CLI rather than using a parser
//! library, so that flag semantics (including `--flag=value`) match exactly.

use std::collections::VecDeque;
use std::process::ExitCode;

use node_semver_rs::constants::RELEASE_TYPES;
use node_semver_rs::semver::IdentifierBase;
use node_semver_rs::{clean, coerce, compare, inc, rcompare, satisfies, valid, Options};

struct Inc {
    value: String,
    maybe_errant_value: Option<String>,
    option: String,
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    run(argv)
}

fn run(args: Vec<String>) -> ExitCode {
    let mut argv: VecDeque<String> = args.into();

    let mut versions: Vec<String> = Vec::new();
    let mut range: Vec<String> = Vec::new();
    let mut inc_arg: Option<Inc> = None;
    let mut loose = false;
    let mut include_prerelease = false;
    let mut do_coerce = false;
    let mut rtl = false;
    let mut identifier: Option<String> = None;
    let mut identifier_base = IdentifierBase::Unset;
    let mut reverse = false;

    if argv.is_empty() {
        help();
        return ExitCode::SUCCESS;
    }

    while let Some(mut a) = argv.pop_front() {
        if let Some(index) = a.find('=') {
            let value = a[index + 1..].to_string();
            a = a[..index].to_string();
            argv.push_front(value);
        }

        match a.as_str() {
            "-rv" | "-rev" | "--rev" | "--reverse" => reverse = true,
            "-l" | "--loose" => loose = true,
            "-p" | "--include-prerelease" => include_prerelease = true,
            "-v" | "--version" => {
                if let Some(v) = argv.pop_front() {
                    versions.push(v);
                }
            }
            "-i" | "--inc" | "--increment" => {
                let next = argv.front().cloned();
                let is_release_type = next
                    .as_deref()
                    .map(|n| RELEASE_TYPES.contains(&n) || n == "release")
                    .unwrap_or(false);
                if is_release_type {
                    inc_arg = Some(Inc {
                        value: argv.pop_front().unwrap(),
                        maybe_errant_value: None,
                        option: a.clone(),
                    });
                } else {
                    inc_arg = Some(Inc {
                        value: "patch".to_string(),
                        maybe_errant_value: next,
                        option: a.clone(),
                    });
                }
            }
            "--preid" => identifier = argv.pop_front(),
            "-r" | "--range" => {
                if let Some(r) = argv.pop_front() {
                    range.push(r);
                }
            }
            "-n" => {
                identifier_base = match argv.pop_front() {
                    None => IdentifierBase::Unset,
                    Some(v) => IdentifierBase::from_cli(&v),
                };
            }
            "-c" | "--coerce" => do_coerce = true,
            "--rtl" => rtl = true,
            "--ltr" => rtl = false,
            "-h" | "--help" | "-?" => {
                help();
                return ExitCode::SUCCESS;
            }
            _ => versions.push(a),
        }
    }

    let options = Options::new()
        .loose(loose)
        .include_prerelease(include_prerelease)
        .rtl(rtl);

    if let Some(inc) = &inc_arg {
        if let Some(errant) = &inc.maybe_errant_value {
            if versions.iter().any(|v| v == errant) && valid(errant, options).is_none() {
                eprintln!(
                    "Invalid value for {}; defaulting to 'patch'. This may become a failure in future major versions.",
                    inc.option
                );
            }
        }
    }

    let mut versions: Vec<String> = versions
        .into_iter()
        .map(|v| {
            if do_coerce {
                coerce(&v, options).map(|c| c.version).unwrap_or(v)
            } else {
                v
            }
        })
        .filter(|v| valid(v, options).is_some())
        .collect();

    if versions.is_empty() {
        return ExitCode::FAILURE;
    }

    if inc_arg.is_some() && (versions.len() != 1 || !range.is_empty()) {
        eprintln!("--inc can only be used on a single version with no range");
        return ExitCode::FAILURE;
    }

    for r in &range {
        versions.retain(|v| satisfies(v, r.as_str(), options));
        if versions.is_empty() {
            return ExitCode::FAILURE;
        }
    }

    // Every entry is known-valid at this point, so the comparisons cannot fail.
    versions.sort_by(|a, b| {
        if reverse {
            rcompare(a.as_str(), b.as_str(), options).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            compare(a.as_str(), b.as_str(), options).unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    for v in &versions {
        let cleaned = clean(v, options);
        let out = match (&inc_arg, cleaned) {
            (Some(i), Some(c)) => inc(
                &c,
                &i.value,
                options,
                identifier.as_deref(),
                identifier_base,
            ),
            (None, cleaned) => cleaned,
            (Some(_), None) => None,
        };
        match out {
            Some(v) => println!("{v}"),
            None => println!("null"),
        }
    }

    ExitCode::SUCCESS
}

fn help() {
    println!(
        r#"SemVer {version}

A JavaScript implementation of the https://semver.org/ specification
Copyright Isaac Z. Schlueter

Usage: semver [options] <version> [<version> [...]]
Prints valid versions sorted by SemVer precedence

Options:
-r --range <range>
        Print versions that match the specified range.

-i --increment [<level>]
        Increment a version by the specified level.  Level can
        be one of: major, minor, patch, premajor, preminor,
        prepatch, prerelease, or release.  Default level is 'patch'.
        Only one version may be specified.

--preid <identifier>
        Identifier to be used to prefix premajor, preminor,
        prepatch or prerelease version increments.

-l --loose
        Interpret versions and ranges loosely

-p --include-prerelease
        Always include prerelease versions in range matching

-c --coerce
        Coerce a string into SemVer if possible
        (does not imply --loose)

--rtl
        Coerce version strings right to left

--ltr
        Coerce version strings left to right (default)

-n <base>
        Base number to be used for the prerelease identifier.
        Can be either 0 or 1, or false to omit the number altogether.
        Defaults to 0.

Program exits successfully if any valid version satisfies
all supplied ranges, and prints all satisfying versions.

If no satisfying versions are found, then exits failure.

Versions are printed in ascending order, so supplying
multiple versions to the utility will just sort them."#,
        version = env!("CARGO_PKG_VERSION")
    );
}
