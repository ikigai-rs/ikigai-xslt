//! A stylesheet is compiled ONCE per process, however many transforms of it arrive at once
//! (ledger #2826).
//!
//! xrust 2.3 interns every XML name through `qualname` 0.0.2, whose interner is one
//! process-wide `RwLock` taken for writing on every clone and drop of a name. Through 0.5.0
//! every concurrent call took its own pooled thread, and each thread compiled the stylesheet
//! again into its own memo, under that lock: eight renders of gonk's stylesheet at once were
//! eight compiles queued on one lock. This test sends eight at once and counts.
//!
//! One `#[test]` in its own binary, on purpose: [`ikigai_xslt::stylesheet_compiles`] is
//! process-wide, so a neighbor test compiling in parallel would move the count.

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// Generous, because nothing here is meant to time out: a debug build on a 2-core runner
/// compiles gonk's stylesheet in several seconds.
const BUDGET: Duration = Duration::from_secs(120);

/// All eight answered within this, from the moment they are released together. The cost is
/// one compile of gonk's 140 KB stylesheet (about 4 s in a debug build on an idle 16-core
/// machine) plus eight warm runs of a one-element page, one after another. The bound leaves a
/// slow shared CI runner several times that; through 0.5.0 the same eight took about 14 s
/// each, and eight compiles.
const ALL_EIGHT_WITHIN: Duration = Duration::from_secs(60);

/// Send `n` transforms of `style` at once, each with its own source, and return the answers
/// and how long the last one took.
fn at_once(n: usize, style: &str, sources: impl Fn(usize) -> String) -> (Vec<String>, Duration) {
    let style: Arc<str> = Arc::from(style);
    let gate = Arc::new(Barrier::new(n + 1));
    let calls: Vec<_> = (0..n)
        .map(|i| {
            let (style, gate, src) = (Arc::clone(&style), Arc::clone(&gate), sources(i));
            std::thread::spawn(move || {
                gate.wait();
                ikigai_xslt::transform_xml_within(&src, &style, false, BUDGET)
            })
        })
        .collect();
    gate.wait();
    let start = Instant::now();
    let answers = calls
        .into_iter()
        .map(|call| call.join().expect("a caller thread").expect("a transform"))
        .collect();
    (answers, start.elapsed())
}

fn small_style(marker: &str) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/"><out mark="{marker}"><xsl:value-of select="doc/@n"/></out></xsl:template>
</xsl:stylesheet>"#
    )
}

#[test]
fn eight_concurrent_transforms_compile_their_stylesheet_once() {
    // 1. gonk's own stylesheet, eight renders at once: one compile, all eight answered.
    let gonk = include_str!("fixtures/gonk.xsl");
    let page = r#"<view:page xmlns:view="urn:iki:gonk:view#" view="ledger" page-url="/l/default" items-url="/items" status="open"/>"#;
    let before = ikigai_xslt::stylesheet_compiles();
    let (answers, took) = at_once(8, gonk, |_| page.to_string());
    let compiles = ikigai_xslt::stylesheet_compiles() - before;
    eprintln!("gonk.xsl: 8 concurrent transforms, {compiles} compile(s), all answered in {took:?}");
    assert_eq!(
        compiles, 1,
        "eight concurrent transforms of one stylesheet compiled it {compiles} times"
    );
    let expected = include_str!("fixtures/gonk.expected.xml").trim_end();
    assert!(
        answers.iter().all(|a| a == expected),
        "every render is the same page"
    );
    assert!(took < ALL_EIGHT_WITHIN, "eight renders took {took:?}");

    // 2. A second stylesheet compiles separately, once, and its answers are its own.
    let (first, edited) = (small_style("first"), small_style("edited"));
    let before = ikigai_xslt::stylesheet_compiles();
    let (answers, _) = at_once(8, &first, |i| format!("<doc n='{i}'/>"));
    assert_eq!(ikigai_xslt::stylesheet_compiles() - before, 1);
    for (i, answer) in answers.iter().enumerate() {
        assert_eq!(answer, &format!("<out mark='first'>{i}</out>"));
    }

    // 3. An edited stylesheet is a different one: it compiles once, and the first stays warm.
    let before = ikigai_xslt::stylesheet_compiles();
    let (answers, _) = at_once(8, &edited, |i| format!("<doc n='{i}'/>"));
    assert_eq!(ikigai_xslt::stylesheet_compiles() - before, 1);
    assert!(answers.iter().all(|a| a.starts_with("<out mark='edited'>")));
    let before = ikigai_xslt::stylesheet_compiles();
    at_once(4, &first, |i| format!("<doc n='{i}'/>"));
    at_once(4, gonk, |_| page.to_string());
    assert_eq!(
        ikigai_xslt::stylesheet_compiles() - before,
        0,
        "both stylesheets were still compiled"
    );
}
