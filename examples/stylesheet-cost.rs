//! Where does an XSLT call's time go — parsing the stylesheet, compiling it, or running
//! it? Ledger #453 reported ~156 ms per `transform_xml` call through gonk's stylesheet
//! with no warm-up across calls; this is the tool that split that number, and the one to
//! re-run when xrust moves.
//!
//!     cargo run --release --example stylesheet-cost -- <stylesheet.xsl> [source.xml]
//!
//! Or generate offline RSS with independently variable item count and CDATA body size:
//!
//!     cargo run --release --example stylesheet-cost -- --rss [items=1000] [body-bytes=850]
//!
//! Source stages separate the guard, parse, evaluation and XML serialization using stock
//! xrust RNode (no ikigai node wrapper or HTML postprocessing). End-to-end rows above them
//! use the crate. Stage setup and teardown are excluded; end-to-end rows include teardown.
//! This is a trusted-input benchmark: direct calls have no deadline and the public API row
//! uses an hour instead of the production 5 s budget. File I/O is outside every timer.
//!
//! With no source document the source is `<doc/>`, which isolates the fixed overhead from
//! anything the data costs. The parse/compile split reaches past this crate's API into
//! `xrust` directly, because `CompiledStylesheet::compile` deliberately does both.

#[path = "rss-cost/mod.rs"]
mod rss_cost;
use rss_cost::{rss, RSS_STYLE};
use std::time::{Duration, Instant};
use xrust::item::{Item, Node, SequenceTrait};
use xrust::parser::xml::parse as xmlparse;
use xrust::parser::ParseError;
use xrust::transform::context::StaticContextBuilder;
use xrust::trees::smite::RNode;
use xrust::xdmerror::Error as XsltError;
use xrust::xslt::from_document;

fn parse_xml(s: &str) -> Result<RNode, XsltError> {
    let doc = RNode::new_document();
    xmlparse(
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
    // Direct xrust and CompiledStylesheet calls need the same stack as production.
    ikigai_xslt::limits::on_xslt_stack(run).expect("benchmark worker");
}

/// Keep both setup and teardown out of a stage's timer. In particular, dropping a
/// source tree must not be mistaken for parsing it, or evaluation for serialization.
fn stage<S, R>(label: &str, mut setup: impl FnMut() -> S, mut work: impl FnMut(&S) -> R) {
    for run in 0..RUNS {
        let state = setup();
        let start = Instant::now();
        let result = work(&state);
        let elapsed = ms(start.elapsed());
        std::hint::black_box(&result);
        println!("  {label:<18} run {run}: {elapsed:8.3} ms");
        drop(result);
        drop(state);
    }
}

fn source_stages(style: &str, src: &str) {
    println!("\nsource stages (stock xrust RNode; setup/teardown excluded):");
    stage(
        "source guard",
        || (),
        |_| ikigai_xslt::limits::check_source(src, "src").expect("source bounds"),
    );
    stage(
        "parse source",
        || (),
        |_| parse_xml(src).expect("parse source"),
    );
    let compiled = from_document(parse_xml(style).unwrap(), None, parse_xml, |_| {
        Ok(String::new())
    })
    .expect("compile stock xrust");
    let evaluate = |srcdoc: &RNode| {
        let mut ctxt = compiled.clone();
        ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
        ctxt.result_document(RNode::new_document());
        let mut stctxt = StaticContextBuilder::new()
            .message(|_| Ok(()))
            .fetcher(|_| {
                Err(XsltError::new(
                    xrust::xdmerror::ErrorKind::NotImplemented,
                    "no fetcher",
                ))
            })
            .parser(|_| {
                Err(XsltError::new(
                    xrust::xdmerror::ErrorKind::NotImplemented,
                    "no parser",
                ))
            })
            .build();
        ctxt.evaluate(&mut stctxt).expect("evaluate")
    };
    stage("evaluate", || parse_xml(src).unwrap(), &evaluate);
    stage(
        "serialize XML",
        || evaluate(&parse_xml(src).unwrap()),
        |seq| seq.to_xml(),
    );
}

fn run() {
    let mut args = std::env::args().skip(1);
    let style_path = args
        .next()
        .expect("usage: stylesheet-cost <stylesheet.xsl> [source.xml]");
    let (style, src) = if style_path == "--rss" {
        let items = args
            .next()
            .map(|s| s.parse().expect("item count"))
            .unwrap_or(1000);
        let body = args
            .next()
            .map(|s| s.parse().expect("body bytes"))
            .unwrap_or(850);
        println!("synthetic RSS: {items} items, {body} CDATA body bytes per item");
        (RSS_STYLE.to_string(), rss(items, body))
    } else {
        let style = std::fs::read_to_string(&style_path).expect("read stylesheet");
        let src = match args.next() {
            Some(path) => std::fs::read_to_string(path).expect("read source"),
            None => "<doc/>".to_string(),
        };
        (style, src)
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
    // A measurement should finish even when it exceeds the production 5 s budget.
    timed("transform_xml", || {
        ikigai_xslt::transform_xml_within(&src, &style, false, Duration::from_secs(3600))
            .expect("transform")
            .len()
    });
    source_stages(&style, &src);
}
