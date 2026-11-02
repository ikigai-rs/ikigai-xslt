//! What do concurrent transforms of ONE stylesheet cost, and how many compiles do they take?
//! The evidence behind ledger #2826, and the tool to re-run when xrust or qualname moves:
//!
//!     cargo run --release --example concurrent-cost -- [stylesheet.xsl] [source.xml] [n]
//!
//! With no arguments it uses gonk's stylesheet (`tests/fixtures/gonk.xsl`, 140 KB) and the
//! ledger page that fixture's test renders, eight calls at once. Four rounds:
//!
//! * COLD, ONE CALL: the memo cleared, a single call. The compile alone, uncontended.
//! * COLD, AT ONCE: the memo cleared, `n` calls released together. What a host pays when a burst of
//!   requests meets a stylesheet it has not compiled yet.
//! * WARM, AT ONCE: `n` more calls released together, with the stylesheet compiled.
//! * WARM, ONE AT A TIME: the same `n` calls from one thread, one after another. If the warm
//!   concurrent round is not faster than this, the calls did not run in parallel.
//!
//! Every call runs with an hour's budget, so the numbers measure the work, not the bound.

use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

const HOUR: Duration = Duration::from_secs(3600);

const LEDGER_PAGE: &str = r#"<view:page xmlns:view="urn:iki:gonk:view#" view="ledger" page-url="/l/default" items-url="/items" status="open"/>"#;

/// `n` calls released together; the time until the last answer, and the slowest call.
fn at_once(n: usize, src: &Arc<str>, style: &Arc<str>) -> (Duration, Duration) {
    let gate = Arc::new(Barrier::new(n + 1));
    let calls: Vec<_> = (0..n)
        .map(|_| {
            let (src, style, gate) = (Arc::clone(src), Arc::clone(style), Arc::clone(&gate));
            std::thread::spawn(move || {
                gate.wait();
                let start = Instant::now();
                ikigai_xslt::transform_xml_within(&src, &style, false, HOUR).expect("transform");
                start.elapsed()
            })
        })
        .collect();
    gate.wait();
    let start = Instant::now();
    let slowest = calls
        .into_iter()
        .map(|c| c.join().expect("caller"))
        .max()
        .unwrap_or_default();
    (start.elapsed(), slowest)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let style: Arc<str> = match args.next() {
        Some(path) => std::fs::read_to_string(path)
            .expect("read the stylesheet")
            .into(),
        None => include_str!("../tests/fixtures/gonk.xsl").into(),
    };
    let src: Arc<str> = match args.next() {
        Some(path) => std::fs::read_to_string(path)
            .expect("read the source")
            .into(),
        None => LEDGER_PAGE.into(),
    };
    let n: usize = args.next().map_or(8, |n| n.parse().expect("n"));
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get());
    println!(
        "{build} build, {cores} cores: {n} transforms of a {} KB stylesheet",
        style.len() / 1024
    );

    ikigai_xslt::clear_stylesheet_cache();
    let start = Instant::now();
    ikigai_xslt::transform_xml_within(&src, &style, false, HOUR).expect("transform");
    println!(
        "  cold, one call       {:>9.1} ms",
        start.elapsed().as_secs_f64() * 1e3
    );

    ikigai_xslt::clear_stylesheet_cache();
    let before = ikigai_xslt::stylesheet_compiles();
    let (all, slowest) = at_once(n, &src, &style);
    println!(
        "  cold, at once        {:>9.1} ms for all, slowest call {:>9.1} ms, {} compile(s)",
        all.as_secs_f64() * 1e3,
        slowest.as_secs_f64() * 1e3,
        ikigai_xslt::stylesheet_compiles() - before
    );

    let before = ikigai_xslt::stylesheet_compiles();
    let (all, slowest) = at_once(n, &src, &style);
    println!(
        "  warm, at once        {:>9.1} ms for all, slowest call {:>9.1} ms, {} compile(s)",
        all.as_secs_f64() * 1e3,
        slowest.as_secs_f64() * 1e3,
        ikigai_xslt::stylesheet_compiles() - before
    );

    let before = ikigai_xslt::stylesheet_compiles();
    let start = Instant::now();
    for _ in 0..n {
        ikigai_xslt::transform_xml_within(&src, &style, false, HOUR).expect("transform");
    }
    println!(
        "  warm, one at a time  {:>9.1} ms for all, {} compile(s)",
        start.elapsed().as_secs_f64() * 1e3,
        ikigai_xslt::stylesheet_compiles() - before
    );
}
