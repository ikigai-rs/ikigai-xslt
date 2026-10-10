//! Caller XML that would overflow the stack is refused, or run on a stack big enough for it
//! (ledger #916).
//!
//! The claim: ~250 nested elements overflow a 2 MiB thread inside xrust in a release build
//! (~24 in debug) and abort the process. Both inputs a caller can supply are XML — the
//! source document and the stylesheet — so both are probed, through `urn:xslt:transform`.
//! A stack overflow aborts the whole process on any thread, so every reproduction runs in a
//! CHILD PROCESS: this test binary re-executed with one probe named in its environment, on a
//! 2 MiB thread, a tokio worker's size. The parent asserts on the child's exit, so an abort
//! reads as a failure naming the thread that overflowed.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request, Verb};
use std::process::Command;
use std::sync::Arc;

const PROBE: &str = "IKIGAI_XSLT_DEPTH_PROBE";

/// A stylesheet that writes one constant: the source is parsed and nothing else is done
/// with it.
const CONSTANT: &str = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/">done</xsl:template>
</xsl:stylesheet>"#;

/// A stylesheet that walks the whole source: the string value of the root.
const STRING_VALUE: &str = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="."/></xsl:template>
</xsl:stylesheet>"#;

/// A stylesheet that copies the whole source into the result and serializes it.
const COPY_OF: &str = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/"><xsl:copy-of select="/*"/></xsl:template>
</xsl:stylesheet>"#;

/// The identity transform: one template application per level of the source.
const IDENTITY: &str = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="@*|node()"><xsl:copy><xsl:apply-templates select="@*|node()"/></xsl:copy></xsl:template>
</xsl:stylesheet>"#;

fn nested(n: usize) -> String {
    format!("{}x{}", "<a>".repeat(n), "</a>".repeat(n))
}

/// A stylesheet whose one template nests `n` literal result elements.
fn nested_style(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/">{}x{}</xsl:template>
</xsl:stylesheet>"#,
        "<b>".repeat(n),
        "</b>".repeat(n)
    )
}

/// A stylesheet whose one XPath nests `n` parentheses.
fn xpath_parens(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="{}1{}"/></xsl:template>
</xsl:stylesheet>"#,
        "(".repeat(n),
        ")".repeat(n)
    )
}

/// A stylesheet whose one XPath is a flat chain of `n` additions.
fn xpath_plus(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="1{}"/></xsl:template>
</xsl:stylesheet>"#,
        "+1".repeat(n)
    )
}

/// One XPath with `n` nested predicates: `*[*[*[…]]]`.
fn xpath_brackets(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="count({}x{})"/></xsl:template>
</xsl:stylesheet>"#,
        "*[".repeat(n),
        "]".repeat(n)
    )
}

/// One XPath of `n` unary minuses: `- - - 1`.
fn xpath_minus(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="{}1"/></xsl:template>
</xsl:stylesheet>"#,
        "- ".repeat(n)
    )
}

/// One location path of `n` steps: `a/a/a…`.
fn xpath_path(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="count(a{})"/></xsl:template>
</xsl:stylesheet>"#,
        "/a".repeat(n)
    )
}

/// An attribute value template on a literal result element nesting `n` parentheses.
fn avt_parens(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/"><b c="{{{}1{}}}"/></xsl:template>
</xsl:stylesheet>"#,
        "(".repeat(n),
        ")".repeat(n)
    )
}

/// A named template that calls itself `n` times: no nesting in the text at all.
fn recursion(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:call-template name="r"><xsl:with-param name="n" select="{n}"/></xsl:call-template></xsl:template>
  <xsl:template name="r"><xsl:param name="n"/><xsl:if test="$n &gt; 0">.<xsl:call-template name="r"><xsl:with-param name="n" select="$n - 1"/></xsl:call-template></xsl:if></xsl:template>
</xsl:stylesheet>"#
    )
}

/// A named template that calls itself `n` times (xrust allows 199), each call nested inside
/// `nest` literal result elements: the result nests `nest × n` deep.
fn nested_recursion(n: usize, nest: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/"><xsl:call-template name="r"><xsl:with-param name="n" select="{n}"/></xsl:call-template></xsl:template>
  <xsl:template name="r"><xsl:param name="n"/><xsl:if test="$n &gt; 0">{}<xsl:call-template name="r"><xsl:with-param name="n" select="$n - 1"/></xsl:call-template>{}</xsl:if></xsl:template>
</xsl:stylesheet>"#,
        "<b>".repeat(nest),
        "</b>".repeat(nest)
    )
}

/// An `xsl:function` that calls itself `n` times, each call inside eight parentheses: the
/// expression's nesting multiplied by the recursion.
fn function_recursion(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:f="urn:f">
  <xsl:output method="text"/>
  <xsl:function name="f:r"><xsl:param name="n"/><xsl:if test="$n &gt; 0"><xsl:sequence select="{}f:r($n - 1){}"/></xsl:if></xsl:function>
  <xsl:template match="/"><xsl:value-of select="count(f:r({n}))"/></xsl:template>
</xsl:stylesheet>"#,
        "(".repeat(8),
        ")".repeat(8)
    )
}

/// `(src, stylesheet)` for one named probe.
fn inputs(case: &str, n: usize) -> (String, String) {
    match case {
        "src-constant" => (nested(n), CONSTANT.to_string()),
        "src-string" => (nested(n), STRING_VALUE.to_string()),
        "src-copy" => (nested(n), COPY_OF.to_string()),
        "src-identity" => (nested(n), IDENTITY.to_string()),
        "style-nested" => ("<doc/>".to_string(), nested_style(n)),
        "style-parens" => ("<doc/>".to_string(), xpath_parens(n)),
        "style-plus" => ("<doc/>".to_string(), xpath_plus(n)),
        "style-brackets" => ("<doc/>".to_string(), xpath_brackets(n)),
        "style-minus" => ("<doc/>".to_string(), xpath_minus(n)),
        "style-path" => ("<doc/>".to_string(), xpath_path(n)),
        "style-avt" => ("<doc/>".to_string(), avt_parens(n)),
        "style-recurse" => ("<doc/>".to_string(), recursion(n)),
        "style-recnest" => ("<doc/>".to_string(), nested_recursion(n, 8)),
        "style-recnest2" => ("<doc/>".to_string(), nested_recursion(n, 2)),
        "content-constant" => (nested(n), CONSTANT.to_string()),
        "lib-src" => (nested(n), CONSTANT.to_string()),
        // An entity whose value is markup: `n` levels from a reference the tag scan cannot see.
        "src-entity-markup" => (
            format!(
                "<!DOCTYPE d [<!ENTITY e \"{}x{}\">]><d>&e;</d>",
                "<a>".repeat(n),
                "</a>".repeat(n)
            ),
            CONSTANT.to_string(),
        ),
        // The declaration RDF/XML uses: a name for a namespace IRI.
        "src-entity-iri" => (
            "<!DOCTYPE d [<!ENTITY rdf \"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">]><d>&rdf;type</d>"
                .to_string(),
            STRING_VALUE.to_string(),
        ),
        "style-fnrec" => ("<doc/>".to_string(), function_recursion(n)),
        // Wide, not deep: `n` sibling elements under one root.
        "src-wide" => (
            format!("<r>{}</r>", "<a/>".repeat(n)),
            STRING_VALUE.to_string(),
        ),
        other => panic!("no probe named {other}"),
    }
}

fn issue(case: &str, n: usize) -> Result<String, String> {
    let (src, style) = inputs(case, n);
    if case.starts_with("lib-") {
        // The library entry point, which hosts like gonk call without a kernel.
        return ikigai_xslt::transform_xml(&src, &style, true);
    }
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let by = if case.starts_with("content-") {
        "content"
    } else {
        "src"
    };
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg(by, ArgRef::Inline(src.into_bytes()))
        .with_arg("stylesheet", ArgRef::Inline(style.into_bytes()));
    block_on(kernel.issue(req, &Capability::root()))
        .map(|rep| String::from_utf8_lossy(&rep.bytes).into_owned())
        .map_err(|e| e.to_string())
}

/// The child's half: inert unless a parent named a probe. Runs it on a 2 MiB thread,
/// prints the outcome, and exits before the harness can.
#[test]
fn probe_child() {
    let Ok(spec) = std::env::var(PROBE) else {
        return;
    };
    let mut parts = spec.split(':');
    let case = parts.next().unwrap().to_string();
    let n = parts.next().unwrap().parse::<usize>().unwrap();
    // A third field, used only by `measure_stack`, sizes the probe thread in KiB.
    let kib = parts.next().map_or(2048, |k| k.parse::<usize>().unwrap());
    let outcome = std::thread::Builder::new()
        .stack_size(kib << 10)
        .spawn(move || match issue(&case, n) {
            Ok(text) => format!("ok {}", text.chars().take(80).collect::<String>()),
            Err(e) => format!("err {}", e.replace('\n', " ")),
        })
        .unwrap()
        .join()
        .unwrap();
    println!("\nOUTCOME {outcome}");
    std::process::exit(0);
}

/// Run one probe in a child: `Ok(outcome)` when it reported, `Err(how it died)` when not.
fn run(case: &str, n: usize) -> Result<String, String> {
    run_on(case, n, 2048)
}

/// [`run`] on a probe thread of `kib` KiB.
fn run_on(case: &str, n: usize, kib: usize) -> Result<String, String> {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "probe_child", "--nocapture", "--test-threads=1"])
        .env(PROBE, format!("{case}:{n}:{kib}"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        let overflow = stderr
            .lines()
            .find(|l| l.contains("overflowed its stack"))
            .unwrap_or(&stderr);
        return Err(format!("{} — {overflow}", out.status));
    }
    let at = stdout
        .find("\nOUTCOME ")
        .ok_or_else(|| format!("reported nothing: {stdout}"))?;
    Ok(stdout[at + "\nOUTCOME ".len()..]
        .lines()
        .next()
        .unwrap_or("")
        .to_string())
}

/// The parent's half for an assertion: the outcome, or a failure naming the abort.
fn probe(case: &str, n: usize) -> String {
    run(case, n).unwrap_or_else(|died| panic!("the `{case}` probe at {n} aborted: {died}"))
}

/// Not a test of the fix: finds, for every probe, the smallest `n` that aborts the child.
/// `cargo test --test xml_depth -- --ignored --nocapture measure`.
#[test]
#[ignore]
fn measure() {
    let cases = std::env::var("XSLT_MEASURE").unwrap_or_else(|_| {
        "src-constant,src-string,src-copy,src-identity,style-nested,style-parens,style-plus"
            .to_string()
    });
    let cap: usize = std::env::var("XSLT_MEASURE_CAP")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1 << 14);
    for case in cases.split(',') {
        let (mut lo, mut hi) = (1usize, 1usize);
        // Grow until it aborts (or reaches the cap), then bisect.
        loop {
            let t = std::time::Instant::now();
            let ok = run(case, hi).is_ok();
            println!(
                "  {case} {hi}: {} in {:?}",
                if ok { "ok" } else { "ABORT" },
                t.elapsed()
            );
            if !ok {
                break;
            }
            lo = hi;
            if hi >= cap {
                break;
            }
            hi = (hi * 2).min(cap);
        }
        if lo == hi {
            println!("{case}: no abort up to {hi}");
            continue;
        }
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if run(case, mid).is_ok() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        println!("{case}: aborts at {hi} ({:?})", run(case, lo));
    }
}

/// Not a test of the fix: for each probe at each size, the smallest probe thread (in KiB,
/// to 16 KiB) it runs on. The slope between sizes is the stack one level costs.
/// `XSLT_MEASURE=src-constant XSLT_MEASURE_N=50,100 cargo test --test xml_depth -- --ignored
/// --nocapture measure_stack`.
#[test]
#[ignore]
fn measure_stack() {
    let cases = std::env::var("XSLT_MEASURE").unwrap_or_else(|_| "src-constant".to_string());
    let sizes = std::env::var("XSLT_MEASURE_N").unwrap_or_else(|_| "50,100".to_string());
    for case in cases.split(',') {
        let mut last: Option<(usize, usize)> = None;
        for n in sizes.split(',').map(|n| n.parse::<usize>().unwrap()) {
            let (mut lo, mut hi) = (16usize, 64usize);
            while run_on(case, n, hi).is_err() {
                lo = hi;
                hi *= 2;
                assert!(hi < 64 << 20, "{case} {n}: no stack up to 64 GiB runs it");
            }
            while hi - lo > 16 {
                let mid = (lo + hi) / 2;
                if run_on(case, n, mid).is_ok() {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let t = std::time::Instant::now();
            let outcome = run_on(case, n, hi);
            println!(
                "  {case} n={n} at {hi} KiB: {:?} in {:?}",
                outcome.map(|o| o.chars().take(60).collect::<String>()),
                t.elapsed()
            );
            let slope = last.map(|(n0, k0)| (hi.saturating_sub(k0) as f64) / ((n - n0) as f64));
            println!("{case} n={n}: needs {hi} KiB; per level since last: {slope:?} KiB");
            last = Some((n, hi));
        }
    }
}

// ------------------------------------------------------------------ the reproduction
//
// Every probe runs on a 2 MiB thread in a child. Before the fix each of these aborted the
// child in a debug build (24 source levels, 22 stylesheet levels, 7 expression levels) and the
// deep ones in a release build too.

/// The bounds, restated so these tests read as numbers. Unit tests in `src/limits.rs` pin the
/// constants to the same values.
const DEPTH: usize = 64;
const EXPR: usize = 32;

#[test]
fn three_hundred_nested_source_elements_are_refused_by_name_and_abort_nothing() {
    for case in ["src-constant", "src-identity", "src-copy"] {
        let outcome = probe(case, 300);
        assert!(
            outcome.starts_with("err invalid argument `src`") && outcome.contains("deeper than 64"),
            "{case}: {outcome}"
        );
    }
    // A piped document arrives as `content=`, and is refused under that name.
    let outcome = probe("content-constant", 300);
    assert!(
        outcome.starts_with("err invalid argument `content`"),
        "{outcome}"
    );
}

#[test]
fn a_stylesheet_nesting_three_hundred_elements_is_refused_by_name() {
    let outcome = probe("style-nested", 300);
    assert!(
        outcome.starts_with("err invalid argument `stylesheet`")
            && outcome.contains("deeper than 64"),
        "{outcome}"
    );
}

#[test]
fn an_expression_nesting_past_the_bound_is_refused_by_name() {
    for case in ["style-parens", "style-brackets", "style-avt"] {
        let outcome = probe(case, EXPR + 1);
        assert!(
            outcome.starts_with("err invalid argument `stylesheet`")
                && outcome.contains("deeper than 32"),
            "{case}: {outcome}"
        );
    }
}

#[test]
fn everything_at_the_bounds_runs_from_a_two_mib_thread() {
    for case in ["src-constant", "src-string", "src-identity", "src-copy"] {
        let outcome = probe(case, DEPTH);
        assert!(outcome.starts_with("ok "), "{case}: {outcome}");
    }
    // `xsl:stylesheet` and `xsl:template` are two of the stylesheet's levels.
    let outcome = probe("style-nested", DEPTH - 2);
    assert!(outcome.starts_with("ok "), "{outcome}");
    let outcome = probe("style-parens", EXPR);
    assert!(outcome.starts_with("ok "), "{outcome}");
    // `count(` is one of the expression's levels, and the template's `{` another.
    for case in ["style-brackets", "style-avt"] {
        let outcome = probe(case, EXPR - 1);
        assert!(outcome.starts_with("ok "), "{case}: {outcome}");
    }
}

#[test]
fn a_template_that_nests_its_result_as_it_recurses_runs() {
    // No nesting in the text deeper than 3, and a result 300 levels deep: no lexical bound
    // sees it, and it overflowed a 2 MiB thread in a debug build. The thread holds it.
    let outcome = probe("style-recnest2", 150);
    assert!(outcome.starts_with("ok <b><b>"), "{outcome}");
}

#[test]
fn xrust_bounds_recursion_itself() {
    // `XRUST_MAX_DEPTH`: the multiplier the thread is sized with. Past it xrust refuses.
    let outcome = probe("style-recurse", 250);
    assert!(outcome.contains("exceeded evaluation depth"), "{outcome}");
    let outcome = probe("style-recurse", 150);
    assert!(outcome.starts_with("ok ."), "{outcome}");
}

#[test]
fn an_entity_that_expands_to_markup_is_refused_and_a_namespace_entity_runs() {
    let outcome = probe("src-entity-markup", 300);
    assert!(
        outcome.starts_with("err invalid argument `src`") && outcome.contains("`<`"),
        "{outcome}"
    );
    let outcome = probe("src-entity-iri", 1);
    assert_eq!(
        outcome,
        "ok http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
    );
}

#[test]
fn the_library_entry_point_is_bounded_too() {
    let outcome = probe("lib-src", DEPTH);
    assert!(outcome.starts_with("ok done"), "{outcome}");
    let outcome = probe("lib-src", 300);
    assert!(
        outcome.starts_with("err invalid argument `src`"),
        "{outcome}"
    );
}
