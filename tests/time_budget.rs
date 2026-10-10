//! A transform is answered within a time budget (ledger #1040).
//!
//! The claim: some shapes are super-linear in xrust and nothing bounded their time. Measured
//! in a release build (`examples/time-cost.rs`, the numbers in `limits::DEFAULT_TIME_BUDGET`'s
//! docs): an XPath of 16,384 `+1` terms compiles in 1.9 s, and a template recursing to build
//! a 1,592-level result — about 600 bytes of stylesheet — runs 3.7 s, growing roughly as the
//! cube of the depth. Inside every stack bound, from any caller that may read.
//!
//! ⚠ ONE test in this binary, on purpose: the count of overdue transforms is process-wide,
//! and a test that leaves work running past its budget would make a neighbor's call refused.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
use ikigai_xslt::limits::{
    max_overdue_transforms, on_xslt_stack_within, overdue_transforms, DEFAULT_TIME_BUDGET,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Wait until no transform is overdue — or fail, naming how many still are.
fn drained() {
    let start = Instant::now();
    while overdue_transforms() > 0 {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "{} transforms still overdue after 60 s",
            overdue_transforms()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A named template that calls itself `n` times, each call inside `nest` literal elements:
/// the result nests `n × nest` deep, from a stylesheet nesting `nest + 4`.
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

#[test]
fn a_transform_is_answered_within_its_budget_and_overdue_work_is_capped() {
    // 1. Past the budget the caller is answered — at the budget, not when the work ends — with
    //    a typed Timeout naming it; the work is counted until it ends, then uncounted.
    let start = Instant::now();
    let late = on_xslt_stack_within(Duration::from_millis(50), || {
        std::thread::sleep(Duration::from_millis(600));
        7
    });
    let waited = start.elapsed();
    assert!(
        matches!(&late, Err(Error::Timeout(m)) if m.contains("50 ms")),
        "{late:?}"
    );
    assert!(
        waited < Duration::from_millis(500),
        "answered after {waited:?}"
    );
    assert_eq!(overdue_transforms(), 1, "the abandoned work is counted");
    drained();

    // 2. Work that ends in time is answered with its value.
    assert_eq!(
        on_xslt_stack_within(Duration::from_secs(30), || 6 * 7).unwrap(),
        42
    );

    // 3. While the most allowed are overdue, a new call is refused at once, transiently, and
    //    without starting anything; once they end, calls run again.
    let most = max_overdue_transforms();
    for _ in 0..most {
        let late = on_xslt_stack_within(Duration::from_millis(10), || {
            std::thread::sleep(Duration::from_millis(1500));
        });
        assert!(matches!(late, Err(Error::Timeout(_))), "{late:?}");
    }
    assert_eq!(overdue_transforms(), most);
    let ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let refused = on_xslt_stack_within(Duration::from_secs(30), move || {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    assert!(
        matches!(&refused, Err(e @ Error::Unavailable(m))
            if e.is_transient() && m.contains("still running")),
        "{refused:?}"
    );
    assert!(
        !ran.load(std::sync::atomic::Ordering::SeqCst),
        "nothing was started"
    );
    drained();
    assert_eq!(
        on_xslt_stack_within(Duration::from_secs(30), || 1).unwrap(),
        1
    );

    // 4. Real xrust work is abandoned the same way: an 800-level recursive result takes
    //    ~0.5 s in release (several seconds in debug), and is answered at 100 ms.
    let style = nested_recursion(100, 8);
    let start = Instant::now();
    let late =
        ikigai_xslt::transform_xml_within("<doc/>", &style, false, Duration::from_millis(100));
    assert!(
        late.as_ref()
            .is_err_and(|e| e.starts_with("timeout:") && e.contains("100 ms")),
        "{late:?}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    drained();

    // 5. LAST, because its work outlives the test: the endpoint applies DEFAULT_TIME_BUDGET.
    //    A 3,184-level recursive result — ~15 s in release — from 1 KB of stylesheet.
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
        .with_arg(
            "stylesheet",
            ArgRef::Inline(nested_recursion(199, 16).into_bytes()),
        );
    let start = Instant::now();
    let late = block_on(kernel.issue(req, &Capability::root()));
    let waited = start.elapsed();
    let budget = format!("{} ms", DEFAULT_TIME_BUDGET.as_millis());
    assert!(
        matches!(&late, Err(Error::Timeout(m)) if m.contains(&budget)),
        "{late:?}"
    );
    assert!(
        waited >= DEFAULT_TIME_BUDGET && waited < DEFAULT_TIME_BUDGET + Duration::from_secs(3),
        "answered after {waited:?}"
    );
}
