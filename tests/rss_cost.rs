//! The offline performance fixture must lift all items independently of ignored body size.
#[path = "../examples/rss-cost/mod.rs"]
mod rss_cost;
use rss_cost::{rss, RSS_STYLE};

#[test]
fn rss_fixture_lifts_each_item_and_ignores_the_body() {
    ikigai_xslt::limits::on_xslt_stack(|| {
        let compiled = ikigai_xslt::CompiledStylesheet::compile(RSS_STYLE).unwrap();
        let expected = "<items><item><title>Entry 0</title><link>https://example.org/0</link><date>Fri, 30 Oct 2026</date></item><item><title>Entry 1</title><link>https://example.org/1</link><date>Fri, 30 Oct 2026</date></item></items>";
        let short = compiled.transform(&rss(2, 0), false).unwrap();
        let long = compiled.transform(&rss(2, 10_000), false).unwrap();
        assert_eq!(short, expected);
        assert_eq!(long, expected);
        assert_eq!(compiled.transform(&rss(0, 0), false).unwrap(), "<items/>");
    })
    .unwrap();
}
