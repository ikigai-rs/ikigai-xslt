//! An XPath expression's bracket nesting is bounded before it is parsed, and what the bound
//! admits parses on a small stack (ikigai-xrust, ledger #3702).
//!
//! The parser descends its whole grammar, by recursion, once per bracket. In xrust 2.3.0 that
//! cost ~150 KiB of stack a bracket in a release build, because every combinator frame held a
//! copy of the ~600-byte parser state, so an RSS stylesheet's `substring(concat(…), …)`
//! overflowed a 1 MiB wasm stack. A stack overflow aborts the process, so each probe runs in a
//! CHILD process: this test binary re-executed with the probe named in its environment.

use std::process::Command;
use xrust::parser::xpath::{parse, MAX_NESTING};
use xrust::trees::smite::RNode;
use xrust::ErrorKind;

const PROBE: &str = "XRUST_NESTING_PROBE";

/// The stack the deepest admitted expression must parse in: 2 MiB in release, a tokio
/// worker's, and 8 MiB with `debug_assertions`, the default main thread's.
const STACK: usize = if cfg!(debug_assertions) { 8 << 20 } else { 2 << 20 };

fn parens(n: usize) -> String {
    format!("{}1{}", "(".repeat(n), ")".repeat(n))
}

fn predicates(n: usize) -> String {
    format!("count({}x{})", "*[".repeat(n - 1), "]".repeat(n - 1))
}

fn calls(n: usize) -> String {
    format!("{}'a'{}", "concat('(', ".repeat(n), ")".repeat(n))
}

/// A match pattern of `n` nested predicates: `a[a[…x]]`.
fn pattern(n: usize) -> String {
    format!("{}x{}", "a[".repeat(n), "]".repeat(n))
}

fn expression(case: &str, n: usize) -> String {
    match case {
        "pattern" => pattern(n),
        "parens" => parens(n),
        "predicates" => predicates(n),
        "calls" => calls(n),
        other => panic!("no probe named {other}"),
    }
}

/// Parse `e` as a pattern for the `pattern` probe, as an expression for every other.
fn compile(case: &str, e: &str) -> Result<(), xrust::Error> {
    if case == "pattern" {
        xrust::Pattern::<RNode>::try_from(e).map(|_| ())
    } else {
        parse::<RNode>(e, None, None).map(|_| ())
    }
}

/// The child's half: inert unless a parent named a probe.
#[test]
fn probe_child() {
    let Ok(spec) = std::env::var(PROBE) else {
        return;
    };
    let (case, n) = spec.split_once(':').unwrap();
    let case = case.to_string();
    let e = expression(&case, n.parse().unwrap());
    let outcome = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || match compile(&case, &e) {
            Ok(()) => "ok".to_string(),
            Err(e) if e.kind == ErrorKind::LimitExceeded => format!("limit {}", e.message),
            Err(e) => format!("err {}", e.message),
        })
        .unwrap()
        .join()
        .unwrap();
    println!("\nOUTCOME {outcome}");
    std::process::exit(0);
}

fn probe(case: &str, n: usize) -> String {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "probe_child", "--nocapture", "--test-threads=1"])
        .env(PROBE, format!("{case}:{n}"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{case} {n}: the child died ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let at = stdout.find("\nOUTCOME ").expect("the child reported");
    stdout[at + "\nOUTCOME ".len()..].lines().next().unwrap().to_string()
}

#[test]
fn the_deepest_admitted_expression_parses_on_a_small_stack() {
    for case in ["parens", "predicates", "calls", "pattern"] {
        assert_eq!(probe(case, MAX_NESTING), "ok", "{case}");
    }
}

#[test]
fn one_bracket_past_the_bound_is_refused_before_parsing() {
    for case in ["parens", "predicates", "calls", "pattern"] {
        let outcome = probe(case, MAX_NESTING + 1);
        assert!(
            outcome.starts_with("limit ") && outcome.contains("deeper than 64"),
            "{case}: {outcome}"
        );
    }
    // Far past it: refused by the scan, which does not recurse, so nothing overflows.
    assert!(probe("parens", 100_000).starts_with("limit "));
}

#[test]
fn brackets_in_strings_and_comments_are_not_counted() {
    let deep = "(".repeat(MAX_NESTING + 1);
    for e in [
        format!("concat('{deep}', 'x')"),
        format!("concat(\"{deep}\", 'x')"),
        format!("concat('it''s {deep}', 'x')"),
        format!("1 (: {deep} don't (: nested :) :) + 1"),
    ] {
        assert!(parse::<RNode>(&e, None, None).is_ok(), "{e}");
    }
    // A comment does not hide what follows it.
    let e = format!("1 (: 'quote :) + {}", parens(MAX_NESTING + 1));
    let refused = parse::<RNode>(&e, None, None).err().expect("refused");
    assert_eq!(refused.kind, ErrorKind::LimitExceeded);
}
