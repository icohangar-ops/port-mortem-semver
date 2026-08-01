//! JSON-lines RPC bridge used by the Node parity adapter.
//!
//! Each line on stdin is `{"op":"<name>","args":[...]}` and each line on stdout
//! is either `{"ok":true,"result":<json>}` or
//! `{"ok":false,"error":"<message>","name":"<TypeError|Error>"}`.
//! Argument shapes mirror the JavaScript call signatures: options may be
//! omitted, `null`, a boolean (loose), or an object with `loose`,
//! `includePrerelease` and `rtl`.

use std::cmp::Ordering;
use std::io::{self, BufRead, Write};

use serde::Deserialize;
use serde_json::{json, Map, Value};

use node_semver_rs::comparator::Comparator;
use node_semver_rs::constants::{RELEASE_TYPES, SEMVER_SPEC_VERSION};
use node_semver_rs::identifiers::Identifier;
use node_semver_rs::range::Range;
use node_semver_rs::semver::{IdentifierBase, SemVer};
use node_semver_rs::{functions as f, ranges_api as r, Options};

#[derive(Deserialize)]
struct Request {
    op: String,
    #[serde(default)]
    args: Vec<Value>,
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Err(e) => json!({ "ok": false, "error": format!("bad request: {e}") }),
            Ok(req) => match dispatch(&req.op, &req.args) {
                Ok(result) => json!({ "ok": true, "result": result }),
                Err(e) => json!({ "ok": false, "error": e.message, "name": e.name }),
            },
        };

        writeln!(out, "{response}")?;
        out.flush()?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// errors
// ---------------------------------------------------------------------------

/// A failure carrying the constructor name the JavaScript original would have
/// used, so the Node adapter can rethrow `TypeError` vs `Error` faithfully.
struct RpcError {
    message: String,
    name: &'static str,
}

impl From<String> for RpcError {
    fn from(message: String) -> Self {
        RpcError {
            message,
            name: "TypeError",
        }
    }
}

impl From<node_semver_rs::SemverError> for RpcError {
    fn from(e: node_semver_rs::SemverError) -> Self {
        RpcError {
            name: e.js_error_name(),
            message: e.to_string(),
        }
    }
}

type RpcResult = Result<Value, RpcError>;

// ---------------------------------------------------------------------------
// argument helpers
// ---------------------------------------------------------------------------

fn arg(args: &[Value], i: usize) -> Option<&Value> {
    args.get(i).filter(|v| !v.is_null())
}

/// `typeof value` in JavaScript, used to reproduce node-semver's TypeError text.
fn js_typeof(v: Option<&Value>) -> &'static str {
    match v {
        None => "undefined",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        _ => "object",
    }
}

/// A version-ish argument. Non-strings produce the exact message `new SemVer`
/// would have thrown, so callers can decide whether to swallow it (like
/// `parse`) or propagate it (like `clean`).
fn need_str(args: &[Value], i: usize) -> Result<String, RpcError> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.clone()),
        // The bridge encodes SemVer instances as objects with a `version` key.
        Some(Value::Object(o)) if o.get("version").and_then(|v| v.as_str()).is_some() => {
            Ok(o["version"].as_str().unwrap().to_string())
        }
        other => Err(format!(
            "Invalid version. Must be a string. Got type \"{}\".",
            js_typeof(other)
        )
        .into()),
    }
}

/// `parse`, `valid` and friends wrap construction in try/catch and yield null.
fn nullable(r: RpcResult) -> RpcResult {
    Ok(r.unwrap_or(Value::Null))
}

/// `parseOptions` for a JSON argument.
fn opts(args: &[Value], i: usize) -> Options {
    match arg(args, i) {
        None => Options::EMPTY,
        Some(Value::Bool(b)) => Options::from(*b),
        Some(Value::Object(o)) => Options::new()
            .loose(truthy(o.get("loose")))
            .include_prerelease(truthy(o.get("includePrerelease")))
            .rtl(truthy(o.get("rtl"))),
        // Any other truthy non-object coerces to `{ loose: true }`.
        Some(v) => {
            if json_truthy(v) {
                Options::LOOSE
            } else {
                Options::EMPTY
            }
        }
    }
}

fn truthy(v: Option<&Value>) -> bool {
    v.map(json_truthy).unwrap_or(false)
}

fn json_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn str_list(args: &[Value], i: usize) -> Result<Vec<String>, RpcError> {
    match arg(args, i) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| match v {
                Value::String(s) => Ok(s.clone()),
                Value::Object(o) => o
                    .get("version")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| "array item must be a version string".to_string().into()),
                other => Ok(other.to_string()),
            })
            .collect(),
        _ => Err(format!("argument {i} must be an array").into()),
    }
}

fn ident_base(v: Option<&Value>) -> IdentifierBase {
    match v {
        None | Some(Value::Null) => IdentifierBase::Unset,
        Some(Value::Bool(false)) => IdentifierBase::False,
        Some(Value::Bool(true)) => IdentifierBase::One,
        Some(Value::String(s)) => IdentifierBase::from_cli(s),
        Some(Value::Number(n)) => {
            if n.as_f64().map(|x| x != 0.0).unwrap_or(false) {
                IdentifierBase::One
            } else {
                IdentifierBase::Zero
            }
        }
        _ => IdentifierBase::Unset,
    }
}

// ---------------------------------------------------------------------------
// result helpers
// ---------------------------------------------------------------------------

fn ord_json(o: Ordering) -> Value {
    json!(match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}

fn ids_json(ids: &[Identifier]) -> Value {
    Value::Array(
        ids.iter()
            .map(|id| match id {
                Identifier::Numeric(n) => json!(n),
                Identifier::Alpha(s) => json!(s),
            })
            .collect(),
    )
}

fn semver_json(v: &SemVer) -> Value {
    let mut m = Map::new();
    m.insert("raw".into(), json!(v.raw));
    m.insert("major".into(), json!(v.major));
    m.insert("minor".into(), json!(v.minor));
    m.insert("patch".into(), json!(v.patch));
    m.insert("prerelease".into(), ids_json(&v.prerelease));
    m.insert("build".into(), json!(v.build));
    m.insert("version".into(), json!(v.version));
    m.insert("loose".into(), json!(v.loose));
    m.insert("includePrerelease".into(), json!(v.include_prerelease));
    Value::Object(m)
}

fn comparator_json(c: &Comparator) -> Value {
    json!({
        "operator": c.operator,
        "value": c.value,
        "semver": match &c.semver {
            None => Value::String("ANY".to_string()),
            Some(sv) => semver_json(sv),
        },
    })
}

fn range_json(r: &Range) -> Value {
    json!({
        "raw": r.raw,
        "range": r.range(),
        "set": r.set.iter().map(|s| Value::Array(s.iter().map(comparator_json).collect())).collect::<Vec<_>>(),
    })
}

fn opt_str(v: Option<String>) -> Value {
    match v {
        Some(s) => Value::String(s),
        None => Value::Null,
    }
}

fn err(e: node_semver_rs::SemverError) -> RpcError {
    e.into()
}

// ---------------------------------------------------------------------------
// dispatch
// ---------------------------------------------------------------------------

fn dispatch(op: &str, args: &[Value]) -> RpcResult {
    match op {
        // --- functions/* ---------------------------------------------------
        "parse" => nullable(need_str(args, 0).map(|v| match f::parse(&v, opts(args, 1)) {
            Some(sv) => semver_json(&sv),
            None => Value::Null,
        })),
        "valid" => nullable(need_str(args, 0).map(|v| opt_str(f::valid(&v, opts(args, 1))))),
        "clean" => {
            // JS calls `version.trim()` before parsing, so a non-string throws.
            let v = need_str(args, 0)
                .map_err(|_| RpcError::from("version.trim is not a function".to_string()))?;
            Ok(opt_str(f::clean(&v, opts(args, 1))))
        }
        "inc" => {
            // `inc` swallows every error and returns null.
            let (version, release) = match (need_str(args, 0), need_str(args, 1)) {
                (Ok(v), Ok(r)) => (v, r),
                _ => return Ok(Value::Null),
            };
            // `inc(version, release, options, identifier, identifierBase)` with
            // the JS overload where `options` may actually be the identifier.
            let (options, identifier, base) = match arg(args, 2) {
                Some(Value::String(s)) => (Options::EMPTY, Some(s.clone()), ident_base(args.get(3))),
                _ => (
                    opts(args, 2),
                    arg(args, 3).and_then(|v| v.as_str().map(|s| s.to_string())),
                    ident_base(args.get(4)),
                ),
            };
            Ok(opt_str(f::inc(
                &version,
                &release,
                options,
                identifier.as_deref(),
                base,
            )))
        }
        "diff" => {
            let a = need_str(args, 0)?;
            let b = need_str(args, 1)?;
            Ok(opt_str(f::diff(&a, &b).map_err(err)?))
        }
        "major" => Ok(json!(f::major(
            need_str(args, 0)?.as_str(),
            opts(args, 1)
        )
        .map_err(err)?)),
        "minor" => Ok(json!(f::minor(
            need_str(args, 0)?.as_str(),
            opts(args, 1)
        )
        .map_err(err)?)),
        "patch" => Ok(json!(f::patch(
            need_str(args, 0)?.as_str(),
            opts(args, 1)
        )
        .map_err(err)?)),
        "prerelease" => Ok(match f::prerelease(&need_str(args, 0)?, opts(args, 1)) {
            Some(ids) => ids_json(&ids),
            None => Value::Null,
        }),
        "compare" => Ok(ord_json(
            f::compare(
                need_str(args, 0)?.as_str(),
                need_str(args, 1)?.as_str(),
                opts(args, 2),
            )
            .map_err(err)?,
        )),
        "rcompare" => Ok(ord_json(
            f::rcompare(
                need_str(args, 0)?.as_str(),
                need_str(args, 1)?.as_str(),
                opts(args, 2),
            )
            .map_err(err)?,
        )),
        "compareLoose" => Ok(ord_json(
            f::compare_loose(need_str(args, 0)?.as_str(), need_str(args, 1)?.as_str())
                .map_err(err)?,
        )),
        "compareBuild" => Ok(ord_json(
            f::compare_build(
                need_str(args, 0)?.as_str(),
                need_str(args, 1)?.as_str(),
                opts(args, 2),
            )
            .map_err(err)?,
        )),
        "sort" => Ok(json!(f::sort(str_list(args, 0)?, opts(args, 1)).map_err(err)?)),
        "rsort" => Ok(json!(f::rsort(str_list(args, 0)?, opts(args, 1)).map_err(err)?)),
        "gt" | "lt" | "eq" | "neq" | "gte" | "lte" => {
            let a = need_str(args, 0)?;
            let b = need_str(args, 1)?;
            let o = opts(args, 2);
            let res = match op {
                "gt" => f::gt(a.as_str(), b.as_str(), o),
                "lt" => f::lt(a.as_str(), b.as_str(), o),
                "eq" => f::eq(a.as_str(), b.as_str(), o),
                "neq" => f::neq(a.as_str(), b.as_str(), o),
                "gte" => f::gte(a.as_str(), b.as_str(), o),
                _ => f::lte(a.as_str(), b.as_str(), o),
            };
            Ok(json!(res.map_err(err)?))
        }
        "cmp" => {
            let a = need_str(args, 0)?;
            let o = need_str(args, 1)?;
            let b = need_str(args, 2)?;
            Ok(json!(f::cmp(&a, &o, &b, opts(args, 3)).map_err(err)?))
        }
        "coerce" => {
            let v = match arg(args, 0) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => return Ok(Value::Null),
            };
            Ok(match f::coerce(&v, opts(args, 1)) {
                Some(sv) => semver_json(&sv),
                None => Value::Null,
            })
        }
        "truncate" => {
            // An unparseable (or non-string) version yields null.
            let (v, t) = match (need_str(args, 0), need_str(args, 1)) {
                (Ok(v), Ok(t)) => (v, t),
                _ => return Ok(Value::Null),
            };
            Ok(opt_str(f::truncate(&v, &t, opts(args, 2))))
        }
        "satisfies" => {
            // `Range#test` short-circuits on a falsy version.
            if !args.first().map(json_truthy).unwrap_or(false) {
                return Ok(json!(false));
            }
            let v = need_str(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(json!(f::satisfies(&v, range.as_str(), opts(args, 2))))
        }

        // --- ranges/* ------------------------------------------------------
        "toComparators" => {
            let range = need_str(args, 0)?;
            Ok(json!(r::to_comparators(range.as_str(), opts(args, 1)).map_err(err)?))
        }
        "maxSatisfying" => {
            let versions = str_list(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(opt_str(r::max_satisfying(&versions, &range, opts(args, 2))))
        }
        "minSatisfying" => {
            let versions = str_list(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(opt_str(r::min_satisfying(&versions, &range, opts(args, 2))))
        }
        "minVersion" => {
            let range = need_str(args, 0)?;
            Ok(
                match r::min_version(range.as_str(), opts(args, 1)).map_err(err)? {
                    Some(sv) => semver_json(&sv),
                    None => Value::Null,
                },
            )
        }
        "validRange" => {
            let range = match arg(args, 0) {
                Some(Value::String(s)) => s.clone(),
                _ => return Ok(Value::Null),
            };
            Ok(opt_str(r::valid_range(range.as_str(), opts(args, 1))))
        }
        "outside" => {
            let v = need_str(args, 0)?;
            let range = need_str(args, 1)?;
            let hilo = need_str(args, 2)?;
            Ok(json!(r::outside(&v, &range, &hilo, opts(args, 3)).map_err(err)?))
        }
        "gtr" => {
            let v = need_str(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(json!(r::gtr(&v, &range, opts(args, 2)).map_err(err)?))
        }
        "ltr" => {
            let v = need_str(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(json!(r::ltr(&v, &range, opts(args, 2)).map_err(err)?))
        }
        "intersects" => {
            let a = need_str(args, 0)?;
            let b = need_str(args, 1)?;
            Ok(json!(
                r::intersects(a.as_str(), b.as_str(), opts(args, 2)).map_err(err)?
            ))
        }
        "simplifyRange" | "simplify" => {
            let versions = str_list(args, 0)?;
            let range = need_str(args, 1)?;
            Ok(json!(
                r::simplify(&versions, &range, opts(args, 2)).map_err(err)?
            ))
        }
        "subset" => {
            let sub = need_str(args, 0)?;
            let dom = need_str(args, 1)?;
            Ok(json!(r::subset(&sub, &dom, opts(args, 2)).map_err(err)?))
        }

        // --- class-ish -----------------------------------------------------
        "semverParse" | "SemVer" => {
            let v = need_str(args, 0)?;
            Ok(semver_json(&SemVer::new(&v, opts(args, 1)).map_err(err)?))
        }
        "semverFormat" | "semverToString" => {
            let v = need_str(args, 0)?;
            let mut sv = SemVer::new(&v, opts(args, 1)).map_err(err)?;
            Ok(json!(sv.format()))
        }
        "semverCompare" | "semverCompareMain" | "semverComparePre" | "semverCompareBuild" => {
            let a = SemVer::new(&need_str(args, 0)?, opts(args, 2)).map_err(err)?;
            let b = SemVer::new(&need_str(args, 1)?, opts(args, 2)).map_err(err)?;
            Ok(ord_json(match op {
                "semverCompare" => a.compare(&b),
                "semverCompareMain" => a.compare_main(&b),
                "semverComparePre" => a.compare_pre(&b),
                _ => a.compare_build(&b),
            }))
        }
        "semverInc" => {
            let mut sv = SemVer::new(&need_str(args, 0)?, opts(args, 4)).map_err(err)?;
            let release = need_str(args, 1)?;
            let identifier = arg(args, 2).and_then(|v| v.as_str().map(|s| s.to_string()));
            let base = ident_base(args.get(3));
            sv.inc(&release, identifier.as_deref(), base).map_err(err)?;
            Ok(semver_json(&sv))
        }
        "rangeParse" | "Range" => {
            let range = need_str(args, 0)?;
            Ok(range_json(&Range::new(&range, opts(args, 1)).map_err(err)?))
        }
        "rangeFormat" | "rangeToString" => {
            let range = need_str(args, 0)?;
            Ok(json!(Range::new(&range, opts(args, 1))
                .map_err(err)?
                .range()))
        }
        "rangeTest" => {
            let range = need_str(args, 0)?;
            let version = need_str(args, 1)?;
            Ok(json!(Range::new(&range, opts(args, 2))
                .map_err(err)?
                .test(&version)))
        }
        "rangeIntersects" => {
            // args 3/4 carry each range's own construction options, which need
            // not match the options handed to `intersects` itself.
            let own = |i: usize| if args.len() > i { opts(args, i) } else { opts(args, 2) };
            let a = Range::new(&need_str(args, 0)?, own(3)).map_err(err)?;
            let b = Range::new(&need_str(args, 1)?, own(4)).map_err(err)?;
            Ok(json!(a.intersects(&b, opts(args, 2)).map_err(err)?))
        }
        // Parse one `||`-free chunk of a range into its comparator list. Unlike
        // `Range`, an empty result is returned as-is (loose mode allows it);
        // the caller reproduces the null-set filtering.
        "parseRangeSet" => {
            let chunk = need_str(args, 0)?;
            let comps = node_semver_rs::range::parse_range(&chunk, opts(args, 1)).map_err(err)?;
            Ok(Value::Array(comps.iter().map(comparator_json).collect()))
        }
        "comparatorParse" | "Comparator" => {
            let c = need_str(args, 0)?;
            Ok(comparator_json(
                &Comparator::new(&c, opts(args, 1)).map_err(err)?,
            ))
        }
        "comparatorTest" => {
            let c = Comparator::new(&need_str(args, 0)?, opts(args, 2)).map_err(err)?;
            Ok(json!(c.test(&need_str(args, 1)?)))
        }
        "comparatorIntersects" => {
            let o = opts(args, 2);
            let own = |i: usize| if args.len() > i { opts(args, i) } else { o };
            let a = Comparator::new(&need_str(args, 0)?, own(3)).map_err(err)?;
            let b = Comparator::new(&need_str(args, 1)?, own(4)).map_err(err)?;
            Ok(json!(a.intersects(&b, o).map_err(err)?))
        }

        // --- internals -----------------------------------------------------
        "compareIdentifiers" | "rcompareIdentifiers" => {
            let a = value_as_identifier_str(args.first());
            let b = value_as_identifier_str(args.get(1));
            let ord = if op == "compareIdentifiers" {
                node_semver_rs::compare_identifiers_str(&a, &b)
            } else {
                node_semver_rs::rcompare_identifiers_str(&a, &b)
            };
            Ok(ord_json(ord))
        }
        "SEMVER_SPEC_VERSION" => Ok(json!(SEMVER_SPEC_VERSION)),
        "RELEASE_TYPES" => Ok(json!(RELEASE_TYPES)),
        "ping" => Ok(json!("pong")),

        other => Err(format!("unknown op: {other}").into()),
    }
}

fn value_as_identifier_str(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}
