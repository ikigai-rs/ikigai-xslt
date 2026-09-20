//! Where does an XSLT call's time go — parsing the stylesheet, compiling it, or running
//! it? Ledger #453 reported ~156 ms per `transform_xml` call through gonk's stylesheet
//! with no warm-up across calls; this is the tool that split that number, and the one to
//! re-run when xrust moves.
//!
//!     cargo run --release --example stylesheet-cost -- <stylesheet.xsl> [source.xml]
//!
//! With no source document the source is `<doc/>`, which isolates the fixed overhead from
//! anything the data costs. The parse/compile split reaches past this crate's API into
//! `xrust` directly, because `CompiledStylesheet::compile` deliberately does both.

use std::time::Instant;
use xrust::item::Node;
use xrust::parser::xml::parse as xmlparse;
use xrust::parser::ParseError;
use xrust::trees::smite::RNode;
use xrust::xdmerror::Error as XsltError;
use xrust::xslt::from_document;

fn parse_xml(s: &str) -> Result<RNode, XsltError> {
    let doc = RNode::new_document();
    xmlparse(
        doc.clone(),
        s,
        Some(|_: &_| Err(ParseError::MissingNameSpace)),
    )?;
    Ok(doc)
}

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

const RUNS: u32 = 5;

/// Time `f` `RUNS` times, printing each run. Every run is printed rather than averaged
/// because the absence of warm-up is the finding: before the memo a compile cost the same
/// the fifth time as the first, which is what tells a fixed cost from a cold cache.
fn timed(label: &str, mut f: impl FnMut() -> usize) {
    timed_after(label, |_| (), move |(), _| f())
}

/// The same, with a per-run `setup` whose cost is deliberately *outside* the timer. That
/// is the only way to price the compile on its own: `from_document` consumes the parsed
/// stylesheet, so every run needs a fresh parse that it should not be charged for.
fn timed_after<S>(
    label: &str,
    mut setup: impl FnMut(u32) -> S,
    mut f: impl FnMut(S, u32) -> usize,
) {
    for run in 0..RUNS {
        let state = setup(run);
        let start = Instant::now();
        let bytes = f(state, run);
        let elapsed = ms(start.elapsed());
        match bytes {
            0 => println!("  {label:<18} run {run}: {elapsed:8.3} ms"),
            n => println!("  {label:<18} run {run}: {elapsed:8.3} ms  ({n} bytes out)"),
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let style_path = args
        .next()
        .expect("usage: stylesheet-cost <stylesheet.xsl> [source.xml]");
    let style = std::fs::read_to_string(&style_path).expect("read stylesheet");
    let src = match args.next() {
        Some(path) => std::fs::read_to_string(path).expect("read source"),
        None => "<doc/>".to_string(),
    };
    println!(
        "stylesheet {style_path} ({} bytes), source {} bytes, {RUNS} runs each\n",
        style.len(),
        src.len()
    );

    timed("parse stylesheet", || {
        let doc = parse_xml(&style).expect("parse stylesheet");
        std::hint::black_box(&doc);
        0
    });

    timed_after(
        "compile",
        |_| parse_xml(&style).expect("parse stylesheet"),
        |doc, _| {
            let ctxt = from_document(doc, None, parse_xml, |_| Ok(String::new())).expect("compile");
            std::hint::black_box(&ctxt);
            0
        },
    );

    // Both of the above together, as the crate spells them.
    timed("compile (crate)", || {
        let compiled = ikigai_xslt::CompiledStylesheet::compile(&style).expect("compile");
        std::hint::black_box(&compiled);
        0
    });

    // The after: one compiled stylesheet, many documents.
    let compiled = ikigai_xslt::CompiledStylesheet::compile(&style).expect("compile");
    timed("reuse compiled", || {
        compiled.transform(&src, false).expect("transform").len()
    });

    // `transform_xml` memoizes the compile per thread, so run 0 pays it and the rest do
    // not. Before the memo, every run cost what run 0 costs.
    ikigai_xslt::clear_stylesheet_cache();
    timed("transform_xml", || {
        ikigai_xslt::transform_xml(&src, &style, false)
            .expect("transform")
            .len()
    });
}
