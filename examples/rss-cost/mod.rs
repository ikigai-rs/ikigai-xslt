use std::fmt::Write as _;

pub const RSS_STYLE: &str = include_str!("items.xsl");

/// Deterministic offline RSS: keep the item count and ignored body size independent.
pub fn rss(items: usize, body_bytes: usize) -> String {
    let mut src = String::from(
        r#"<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/"><channel>"#,
    );
    let body = "x".repeat(body_bytes);
    for i in 0..items {
        write!(src, "<item><title>Entry {i}</title><link>https://example.org/{i}</link><pubDate>Fri, 30 Oct 2026 12:00:00 GMT</pubDate><content:encoded><![CDATA[<p>{body}</p>]]></content:encoded></item>").unwrap();
    }
    src.push_str("</channel></rss>");
    src
}
