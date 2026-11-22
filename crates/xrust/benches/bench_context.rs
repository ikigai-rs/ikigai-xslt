//! ikigai (ledger #4053): what a path step costs as a stylesheet grows.
//!
//! A path step derives a new `Context` from the old one, so whatever `Context` holds by value is
//! copied and dropped once per step. This bench runs one match template whose rows evaluate four
//! multi-step paths, over a document of `ROWS` rows, through stylesheets that add N named
//! templates, N stylesheet variables or N key declarations. None of them is ever called, read or
//! used, so the time per row should not depend on N.
//!
//! `cargo bench -p ikigai-xrust --bench bench_context`; divide the reported time by `ROWS` for
//! the time per row.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use xrust::item::{Item, Node};
use xrust::parser::ParseError;
use xrust::parser::xml::parse;
use xrust::transform::context::{Context, StaticContextBuilder};
use xrust::trees::smite::RNode;
use xrust::xdmerror::{Error, ErrorKind};
use xrust::xslt::from_document;

const ROWS: usize = 200;

fn make_from_str(s: &str) -> Result<RNode, Error> {
    let doc = RNode::new_document();
    parse(
        doc.clone(),
        s,
        Some(|_: &_| {
            Err(ParseError::MissingNameSpace {
                prefix: None,
                nsuri: None,
                message: None,
            })
        }),
    )?;
    Ok(doc)
}

fn source() -> String {
    let mut s = String::from("<doc>");
    for i in 0..ROWS {
        s.push_str(&format!(
            "<row><a><b><c>c{i}</c><d>d{i}</d></b></a><e><f>f{i}</f></e></row>"
        ));
    }
    s.push_str("</doc>");
    s
}

/// The stylesheet, with `extra` (N unused declarations) beside its two match templates.
fn stylesheet(extra: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
<xsl:output method="xml"/>
{extra}
<xsl:template match="/"><out><xsl:apply-templates select="doc/row"/></out></xsl:template>
<xsl:template match="row"><r><xsl:value-of select="a/b/c"/>,<xsl:value-of select="a/b/d"/>,<xsl:value-of select="e/f"/>,<xsl:value-of select="count(a/b/*)"/></r></xsl:template>
</xsl:stylesheet>"#
    )
}

fn named_templates(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                r#"<xsl:template name="t{i}"><xsl:param name="p"/><div class="c{i}"><xsl:if test="$p"><span><xsl:value-of select="concat($p, '-{i}')"/></span></xsl:if></div></xsl:template>"#
            )
        })
        .collect()
}

fn variables(n: usize) -> String {
    (0..n)
        .map(|i| format!(r#"<xsl:variable name="g{i}" select="concat('v', '{i}')"/>"#))
        .collect()
}

fn keys(n: usize) -> String {
    (0..n)
        .map(|i| format!(r#"<xsl:key name="k{i}" match="row" use="e/f"/>"#))
        .collect()
}

fn compile(extra: &str) -> Context<RNode> {
    let styledoc = make_from_str(&stylesheet(extra)).expect("unable to parse stylesheet");
    from_document(styledoc, None, make_from_str, |_| {
        Err(Error::new(ErrorKind::NotImplemented, "no modules"))
    })
    .expect("unable to compile stylesheet")
}

fn run(compiled: &Context<RNode>, src: &RNode) -> RNode {
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "no fetcher")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "no parser")))
        .build();
    let mut ctxt = compiled.clone();
    ctxt.context(vec![Item::Node(src.clone())], 0);
    let rd = RNode::new_document();
    ctxt.result_document(rd.clone());
    ctxt.evaluate(&mut stctxt)
        .expect("unable to evaluate stylesheet");
    rd
}

fn path_steps(c: &mut Criterion) {
    let src = make_from_str(&source()).expect("unable to parse source");
    let mut group = c.benchmark_group("context-per-step");
    group.sample_size(20);
    let kinds: [(&str, fn(usize) -> String); 3] = [
        ("named-templates", named_templates),
        ("variables", variables),
        ("keys", keys),
    ];
    for (kind, extra) in kinds {
        for n in [0usize, 10, 40] {
            let compiled = compile(&extra(n));
            // The answer does not depend on N.
            let first = run(&compiled, &src).first_child().expect("no output");
            assert_eq!(
                first.first_child().expect("no row").to_string(),
                "c0,d0,f0,2"
            );
            group.bench_with_input(BenchmarkId::new(kind, n), &n, |b, _| {
                b.iter(|| black_box(run(&compiled, &src)))
            });
        }
    }
    group.finish();
}

criterion_group!(benches, path_steps);
criterion_main!(benches);
