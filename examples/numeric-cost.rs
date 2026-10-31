//! Price the numeric operand wrapper on a synthetic chart, without network inputs.
//! Run: cargo run --release --example numeric-cost -- [points=1000]
//!
//! Stock xrust rows differ only by the numeric wrappers. The scalar-only control
//! deliberately works without the adapter; unadapted node arithmetic is incorrect.
//! Crate rows measure the production adapter. All timers include teardown; evaluation
//! excludes source parsing and XML serialization. Five runs are printed, not asserted.
use std::time::Instant;
use xrust::{
    item::{Item, Node, SequenceTrait},
    parser::{xml::parse, ParseError},
    transform::context::{Context, StaticContextBuilder},
    trees::smite::RNode,
    xdmerror::Error,
    xslt::from_document,
};

fn xml(source: &str) -> Result<RNode, Error> {
    parse(
        RNode::new_document(),
        source,
        Some(|_: &_| {
            Err(ParseError::MissingNameSpace {
                prefix: None,
                nsuri: None,
                message: None,
            })
        }),
    )
}

// This constructs one known fixture, not an XPath parser. Every binary operand
// gets exactly the wrapper numeric_values emits, including intermediate results.
fn binary(left: &str, op: &str, right: &str, wrapped: bool) -> String {
    let operand = |s: &str| {
        if wrapped {
            format!("normalize-space(string(({s})[1]))")
        } else {
            s.to_string()
        }
    };
    format!("({} {op} {})", operand(left), operand(right))
}

fn stylesheet(wrapped: bool) -> String {
    let op = |a: &str, b: &str, c: &str| binary(a, b, c, wrapped);
    let x = op(
        &op("string(@i)", "-", "1"),
        "div",
        &op("count(/chart/p)", "-", "1"),
    );
    let x = op(&x, "*", "800");
    let y = op("400", "-", &op(&op("string(@i)", "mod", "101"), "*", "4"));
    let radius = op("2", "+", &op("string(@i)", "mod", "3"));
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:output method="xml"/><xsl:template match="/"><svg width="800" height="400"><xsl:for-each select="/chart/p"><circle cx="{{{x}}}" cy="{{{y}}}" r="{{{radius}}}"/></xsl:for-each></svg></xsl:template></xsl:stylesheet>"#
    )
}

fn evaluate(compiled: &Context<RNode>, source: RNode) -> xrust::item::Sequence<RNode> {
    let mut context = compiled.clone();
    context.context(vec![Item::Node(source)], 0);
    context.result_document(RNode::new_document());
    let mut static_context = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| {
            Err(Error::new(
                xrust::xdmerror::ErrorKind::NotImplemented,
                "no fetcher",
            ))
        })
        .parser(|_| {
            Err(Error::new(
                xrust::xdmerror::ErrorKind::NotImplemented,
                "no parser",
            ))
        })
        .build();
    context.evaluate(&mut static_context).expect("evaluate")
}

fn timed(label: &str, mut run: impl FnMut()) {
    for i in 0..5 {
        let start = Instant::now();
        run();
        println!(
            "{label:<26} run {i}: {:9.3} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

fn run() {
    let points: usize = std::env::args()
        .nth(1)
        .map(|n| n.parse().expect("points"))
        .unwrap_or(1000);
    assert!(points > 1);
    let mut source = String::from("<chart>");
    for i in 1..=points {
        use std::fmt::Write;
        write!(source, "<p i='{i}'/>").unwrap();
    }
    source.push_str("</chart>");
    let raw = stylesheet(false);
    let wrapped = stylesheet(true);
    println!(
        "{points} points, {} source bytes, {} raw / {} wrapped stylesheet bytes",
        source.len(),
        raw.len(),
        wrapped.len()
    );
    println!(
        "18 operand wrappers/point; {} wrapper evaluations/transform",
        points * 18
    );
    let compile =
        |s: &str| from_document(xml(s).unwrap(), None, xml, |_| Ok(String::new())).unwrap();
    let raw_compiled = compile(&raw);
    let wrapped_compiled = compile(&wrapped);
    let expected = evaluate(&raw_compiled, xml(&source).unwrap()).to_xml();
    assert_eq!(
        expected,
        evaluate(&wrapped_compiled, xml(&source).unwrap()).to_xml()
    );
    let crate_compiled = ikigai_xslt::CompiledStylesheet::compile(&raw).unwrap();
    assert_eq!(expected, crate_compiled.transform(&source, false).unwrap());
    assert_eq!(expected.matches("<circle").count(), points);
    println!(
        "identical stock raw / stock wrapped / crate output: {} bytes",
        expected.len()
    );
    for (label, style, compiled) in [
        ("raw", &raw, &raw_compiled),
        ("wrapped", &wrapped, &wrapped_compiled),
    ] {
        timed(&format!("stock {label} compile"), || {
            std::hint::black_box(compile(style));
        });
        let doc = xml(&source).unwrap();
        timed(&format!("stock {label} evaluate"), || {
            std::hint::black_box(evaluate(compiled, doc.clone()));
        });
        timed(&format!("stock {label} transform"), || {
            std::hint::black_box(evaluate(compiled, xml(&source).unwrap()).to_xml());
        });
    }
    timed("crate compile", || {
        std::hint::black_box(ikigai_xslt::CompiledStylesheet::compile(&raw).unwrap());
    });
    timed("crate reused transform", || {
        std::hint::black_box(crate_compiled.transform(&source, false).unwrap());
    });
}

fn main() {
    ikigai_xslt::limits::on_xslt_stack(run).expect("benchmark worker");
}
