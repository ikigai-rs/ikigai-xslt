//! How long does a transform take — for what real hosts send, and for what a caller could
//! send? The evidence behind [`ikigai_xslt::limits::DEFAULT_TIME_BUDGET`] (ledger #1040), and
//! the tool to re-run when xrust moves:
//!
//!     cargo run --release --example time-cost -- [gonk.xsl] [cms-web styles dir]
//!
//! The real inputs are optional because they live in sibling repos: gonk's 98 KB stylesheet
//! (`ikigai-gonk/web/gonk.xsl`) through the synthetic queue page of gonk's own
//! `examples/xslbench.rs` (its `full` row shape, copied here), and the reading room's
//! `catalog.xsl` (`ikigai-cms-web/styles/`) through RDF/XML shaped like a room page (60
//! resources a page). Every timing runs with no budget at all
//! ([`ikigai_xslt::transform_xml_within`] with an hour), so it measures the work, not the
//! bound.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

const HOUR: Duration = Duration::from_secs(3600);

/// One transform of `src` through `style`, timed twice: COLD (the memo cleared first, so the
/// stylesheet's parse and compile are in the number — what a caller sending a new stylesheet
/// costs) and WARM (the compile memoized — what a host re-rendering its own stylesheet costs).
fn time(label: &str, src: &str, style: &str, text: bool) {
    let run = || {
        let start = Instant::now();
        let outcome = ikigai_xslt::transform_xml_within(src, style, text, HOUR);
        (start.elapsed().as_secs_f64() * 1000.0, outcome)
    };
    ikigai_xslt::clear_stylesheet_cache();
    let (cold, outcome) = run();
    let (warm, _) = run();
    let what = match outcome {
        Ok(out) => format!("{} KB out", out.len() / 1024),
        Err(e) => format!("err {}", e.chars().take(70).collect::<String>()),
    };
    println!(
        "  {label:<44} {:>5} KB in  cold {cold:>9.1} ms  warm {warm:>9.1} ms  {what}",
        (src.len() + style.len()) / 1024
    );
}

fn text_style(select: &str) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="{select}"/></xsl:template>
</xsl:stylesheet>"#
    )
}

/// A named template that calls itself `n` times, each call inside `nest` literal elements.
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

fn attacks() {
    println!("caller-supplied shapes, all inside the stack bounds:");
    for n in [1024usize, 2048, 4096, 8192, 16384] {
        let style = text_style(&format!("1{}", "+1".repeat(n)));
        time(&format!("XPath `1+1+…`, {n} terms"), "<doc/>", &style, true);
    }
    for (n, nest) in [(100usize, 8usize), (150, 8), (199, 8)] {
        time(
            &format!("recursive template, {}-level result", n * nest),
            "<doc/>",
            &nested_recursion(n, nest),
            false,
        );
    }
    for n in [8192usize, 65536] {
        let src = format!("<r>{}</r>", "<a/>".repeat(n));
        time(
            &format!("{n} sibling elements, string value"),
            &src,
            &text_style("."),
            true,
        );
    }
    for n in [250usize, 500, 1000] {
        let src = format!("<r>{}</r>", "<a/>".repeat(n));
        time(
            &format!("`count(//*[count(//*) &gt; 0])`, {n} elements"),
            &src,
            &text_style("count(//*[count(//*) &gt; 0])"),
            true,
        );
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// gonk's `examples/xslbench.rs` `page(n, "none", "full")`: `n` queue rows with the decide
/// form and its option lists.
fn gonk_page(n: usize) -> String {
    let mut s = String::new();
    s.push_str(r#"<view:page xmlns:view="urn:iki:gonk:view#" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:ledger="https://ikigai-rs.dev/ns/ledger#" view="queue" full="true" title="Queue" page-url="/queue">"#);
    s.push_str(r#"<view:ledger name="default" href="/l/default" current="true"/>"#);
    s.push_str(r#"<view:queue href="/queue" depth-url="/queue/depth" every="10s"/>"#);
    s.push_str(r#"<view:state name="pending" href="/queue" rows-url="/queue?rows" current="true"/><view:state name="published" href="/queue?state=published" rows-url="/queue?state=published&amp;rows"/>"#);
    s.push_str(r#"<view:scope name="serious" href="/queue" rows-url="/queue?rows" label="serious only" current="true"/>"#);
    let body = "The comment above documents a hazard that the code below does not guard against: a caller holding no authority reaches the write path when the host name is unset, and nothing in this branch refuses it. Consider checking the origin before the capability is minted, or say in the doc comment why the ordering is safe.";
    for i in 0..n {
        let _ = write!(
            s,
            r#"<view:finding id="f{i:06}" state="pending" severity="major" severity-label="major" repo="ikigai-cli" where="crates/ikigai-embedded/src/lib.rs:{line}" browse-href="/browse/urn:repo:ikigai-cli:file:crates/ikigai-embedded/src/lib.rs" provenance="minted by review-v5@coder · 2026-09-25 18:04 UTC · pass 01m3cvtjzmyby3f6" reanchored="false" orphaned="false">"#,
            line = 100 + i * 7
        );
        let _ = write!(s, "<view:body>{}</view:body>", esc(body));
        let _ = write!(
            s,
            "<view:quote>{}</view:quote>",
            esc("    if !host_is_ours(request.header(\"host\"), door.port) {")
        );
        let _ = write!(
            s,
            r#"<view:decide action="/queue/decide" id="f{i:06}" state="pending" repo="ikigai-cli" severity="major" scope="serious">"#
        );
        for (w, sel) in [
            ("critical", ""),
            ("major", " selected='true'"),
            ("minor", ""),
            ("info", ""),
            ("praise", ""),
        ] {
            let _ = write!(
                s,
                "<view:severity-option value=\"{w}\"{sel}>{w}</view:severity-option>"
            );
        }
        for w in ["publish", "decline"] {
            let _ = write!(
                s,
                "<view:decision-option value=\"{w}\">{w}</view:decision-option>"
            );
        }
        for w in ["misread", "restates", "no-issue", "wont-fix", "duplicate"] {
            let _ = write!(
                s,
                "<view:reason-option value=\"{w}\" title=\"why\">{w}</view:reason-option>"
            );
        }
        s.push_str("</view:decide></view:finding>");
    }
    s.push_str("</view:page>");
    s
}

/// RDF/XML shaped like a reading-room page: `n` resources, each with a kind, a title, an
/// identifier, two creators and four subjects.
fn cms_page(n: usize) -> String {
    let mut s = String::from(
        r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:cms="https://ikigai-rs.dev/ns/cms#" xmlns:dc="http://purl.org/dc/elements/1.1/">"#,
    );
    for i in 0..n {
        let _ = write!(
            s,
            r#"<rdf:Description rdf:about="urn:cms:book:{i}"><cms:kind>book</cms:kind><dc:title>A Title Of Some Length, Volume {i}</dc:title><dc:identifier>https://example.org/book/{i}</dc:identifier><dc:creator>Author One</dc:creator><dc:creator>Author Two</dc:creator><dc:subject>rdf</dc:subject><dc:subject>semantics</dc:subject><dc:subject>roc</dc:subject><dc:subject>rust</dc:subject></rdf:Description>"#
        );
    }
    s.push_str("</rdf:RDF>");
    s
}

fn main() {
    let mut args = std::env::args().skip(1);
    let gonk = args.next();
    let cms = args.next();
    attacks();
    if let Some(path) = gonk {
        let style = std::fs::read_to_string(&path).expect("read gonk.xsl");
        println!(
            "gonk.xsl ({} KB), xslbench `full` queue rows:",
            style.len() / 1024
        );
        for n in [10usize, 25, 50, 100] {
            time(
                &format!("{n} queue rows in one document"),
                &gonk_page(n),
                &style,
                false,
            );
        }
    }
    if let Some(dir) = cms {
        let style = std::fs::read_to_string(format!("{dir}/catalog.xsl")).expect("catalog.xsl");
        println!("cms-web catalog.xsl ({} KB):", style.len() / 1024);
        for n in [60usize, 600, 2000] {
            time(&format!("{n} resources"), &cms_page(n), &style, false);
        }
    }
}
