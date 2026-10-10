//! `xsl:output method="html"` serializes HTML, not XML (ledger #193, part 3).
//!
//! The claim, reproduced on 0.2.1 before the change: a `method="html"` stylesheet came out
//! as XML, `<script src='a.js'/>`, which a browser reads as an open script element that
//! swallows the rest of the page. Every consumer that served the result re-invented a pass to
//! fix it (gonk's `render::html`). Since 0.3.0 the crate does it, in
//! `CompiledStylesheet::transform` and so in `transform_xml` and the endpoint alike.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request, Verb};
use ikigai_xslt::{transform_xml, CompiledStylesheet};
use std::sync::Arc;

const SRC: &str = "<doc><item>x</item></doc>";

fn style(method: Option<&str>, template: &str) -> String {
    let output = method
        .map(|m| format!(r#"<xsl:output method="{m}"/>"#))
        .unwrap_or_default();
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">{output}<xsl:template match="/">{template}</xsl:template></xsl:stylesheet>"#
    )
}

#[test]
fn an_empty_non_void_element_keeps_its_end_tag_and_a_void_one_has_none() {
    let out = transform_xml(
        SRC,
        &style(
            Some("html"),
            r#"<html><head><meta charset="utf-8"/><link rel="x" href="y"/><script src="a.js"></script><style/></head><body><div class="a"/><textarea name="t"></textarea><span/><br/><hr/><img src="i"/><input type="text"/><p><xsl:value-of select="doc/item"/></p><BR/><Div/></body></html>"#,
        ),
        false,
    )
    .expect("renders");
    assert_eq!(
        out,
        "<html><head><meta charset='utf-8'/><link href='y' rel='x'/><script src='a.js'></script><style></style></head>\
         <body><div class='a'></div><textarea name='t'></textarea><span></span><br/><hr/><img src='i'/><input type='text'/><p>x</p><BR/><Div></Div></body></html>"
    );
}

/// An element in a namespace is XML, as XSLT 1.0 §16.2 says: inline SVG keeps `<path/>`, which
/// an HTML parser reads correctly inside foreign content.
#[test]
fn an_element_in_a_namespace_is_serialized_as_xml() {
    let out = transform_xml(
        SRC,
        &style(
            Some("html"),
            r#"<div><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><path d="M0 0"/><g/></svg><i/></div>"#,
        ),
        false,
    )
    .expect("renders");
    assert_eq!(
        out,
        "<div><svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0'/><g/></svg><i></i></div>"
    );
}

/// Only the empty-element case changes: escaping and quoting are xrust's, byte for byte, so
/// a consumer's later pass (gonk's link pass reads `&lt;`, `&apos;`, `&quot;`) sees what it saw.
#[test]
fn escaping_is_unchanged() {
    let out = transform_xml(
        "<doc t=\"a &lt; b &amp; 'c' &quot;d&quot;\"/>",
        &style(
            Some("html"),
            r#"<p title="{doc/@t}"><xsl:value-of select="doc/@t"/></p>"#,
        ),
        false,
    )
    .expect("renders");
    assert_eq!(
        out,
        "<p title='a &lt; b &amp; &apos;c&apos; &quot;d&quot;'>a &lt; b &amp; &apos;c&apos; &quot;d&quot;</p>"
    );
}

/// `method="xml"` and `method="text"` are untouched.
#[test]
fn other_methods_are_untouched() {
    let template = r#"<p><script src="a.js"></script><br/></p>"#;
    assert_eq!(
        transform_xml(SRC, &style(Some("xml"), template), false).as_deref(),
        Ok("<p><script src='a.js'/><br/></p>")
    );
    assert_eq!(
        transform_xml(
            SRC,
            &style(
                Some("text"),
                "<p><b/><xsl:value-of select=\"doc/item\"/></p>"
            ),
            true
        )
        .as_deref(),
        Ok("x")
    );
}

/// With no `xsl:output`, XSLT 1.0 §16's default applies: html when the result's first element
/// is `html` in no namespace (any case), xml otherwise.
#[test]
fn with_no_output_method_an_html_root_means_html() {
    assert_eq!(
        transform_xml(SRC, &style(None, "<HTML><body/></HTML>"), false).as_deref(),
        Ok("<HTML><body></body></HTML>")
    );
    assert_eq!(
        transform_xml(SRC, &style(None, "<page><body/></page>"), false).as_deref(),
        Ok("<page><body/></page>")
    );
}

/// The serialization adds a node to each empty element of the RESULT tree. That tree is the
/// run's own, so a compiled stylesheet reused for many runs answers the same every time, and
/// the same as a fresh compile.
#[test]
fn a_reused_compiled_stylesheet_answers_the_same_every_run() {
    let s = style(
        Some("html"),
        r#"<div><script src="a.js"></script><xsl:for-each select="doc/item"><span/></xsl:for-each></div>"#,
    );
    let compiled = CompiledStylesheet::compile(&s).expect("compile");
    let first = compiled.transform(SRC, false).expect("first");
    assert_eq!(
        first,
        "<div><script src='a.js'></script><span></span></div>"
    );
    for _ in 0..3 {
        assert_eq!(
            compiled.transform(SRC, false).as_deref(),
            Ok(first.as_str())
        );
    }
    assert_eq!(transform_xml(SRC, &s, false).as_deref(), Ok(first.as_str()));
}

/// The endpoint serializes the same way, and labels it `text/html`.
#[test]
fn the_endpoint_serves_html() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(SRC.as_bytes().to_vec()))
        .with_arg(
            "stylesheet",
            ArgRef::Inline(
                style(Some("html"), r#"<p><script src="a.js"></script></p>"#).into_bytes(),
            ),
        );
    let rep = block_on(kernel.issue(req, &Capability::root())).expect("renders");
    assert!(
        rep.repr_type.media_type.starts_with("text/html"),
        "{}",
        rep.repr_type.media_type
    );
    assert_eq!(
        String::from_utf8(rep.bytes).unwrap(),
        "<p><script src='a.js'></script></p>"
    );
}

/// Not done, on purpose, and pinned so the day it changes is a decision: XSLT 1.0 §16.2 writes
/// `script` and `style` content unescaped, and this crate (like xrust) escapes it, so an
/// INLINE script with a quote or a `<` in it is broken. See `src/html.rs`.
#[test]
fn inline_script_text_is_still_escaped() {
    let out = transform_xml(
        SRC,
        &style(
            Some("html"),
            r#"<script>if (a &lt; b) { x = 'y'; }</script>"#,
        ),
        false,
    )
    .expect("renders");
    assert_eq!(out, "<script>if (a &lt; b) { x = &apos;y&apos;; }</script>");
}
