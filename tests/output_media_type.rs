use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request, Verb};
use std::sync::Arc;

fn result(output: &str, template: &str, as_media: Option<&str>) -> ikigai_core::Representation {
    let stylesheet = format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">{output}<xsl:template match="/">{template}</xsl:template></xsl:stylesheet>"#
    );
    let mut request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(stylesheet.into_bytes()));
    if let Some(media) = as_media {
        request = request.with_arg("as", ArgRef::Inline(media.as_bytes().to_vec()));
    }
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    block_on(kernel.issue(request, &Capability::root())).expect("transforms")
}

#[test]
fn explicit_media_type_labels_text_output() {
    let rep = result(
        r#"<xsl:output method="text" media-type="application/n-triples"/>"#,
        "<xsl:text>&lt;urn:s&gt; &lt;urn:p&gt; &lt;urn:o&gt; .</xsl:text>",
        None,
    );
    assert_eq!(rep.repr_type.media_type, "application/n-triples");
    assert_eq!(rep.bytes, b"<urn:s> <urn:p> <urn:o> .");
}

#[test]
fn method_implies_media_type_when_none_is_declared() {
    for (output, template, media) in [
        (r#"<xsl:output method="text"/>"#, "text", "text/plain"),
        (
            r#"<xsl:output method="xml"/>"#,
            "<html/>",
            "application/xml",
        ),
        (r#"<xsl:output method="html"/>"#, "<page/>", "text/html"),
        ("", "<html/>", "text/html"),
        ("", "<page/>", "application/xml"),
        ("", "text", "application/xml"),
        (
            r#"<xsl:output omit-xml-declaration="yes"/>"#,
            "<page/>",
            "application/xml",
        ),
    ] {
        assert_eq!(result(output, template, None).repr_type.media_type, media);
    }
}

#[test]
fn caller_media_type_overrides_stylesheet() {
    let rep = result(
        r#"<xsl:output method="text" media-type="application/n-triples"/>"#,
        "text",
        Some("application/example"),
    );
    assert_eq!(rep.repr_type.media_type, "application/example");
    assert_eq!(rep.bytes, b"text");
}

#[test]
fn media_type_without_method_keeps_markup_serialization() {
    let rep = result(
        r#"<xsl:output media-type="application/example+xml"/>"#,
        "<page/>",
        None,
    );
    assert_eq!(rep.repr_type.media_type, "application/example+xml");
    assert_eq!(rep.bytes, b"<page/>");
}

#[test]
fn stylesheet_media_type_only_labels_the_serialized_result() {
    let rep = result(
        r#"<xsl:output method="xml" media-type="text/plain"/>"#,
        "<page>text</page>",
        None,
    );
    assert_eq!(rep.repr_type.media_type, "text/plain");
    assert_eq!(rep.bytes, b"<page>text</page>");
}

#[test]
fn media_type_parameters_describe_the_utf8_bytes() {
    for (output, as_media) in [
        (
            r#"<xsl:output method='text' media-type='text/plain; profile="A;B"; CHARSET=iso-8859-1'/>"#,
            None,
        ),
        (
            "<xsl:output method='text'/>",
            Some(r#"text/plain; profile="A;B"; CHARSET=iso-8859-1"#),
        ),
    ] {
        let rep = result(output, "café", as_media);
        assert_eq!(rep.bytes, "café".as_bytes());
        assert_eq!(
            rep.repr_type.canonical(),
            r#"text/plain;charset=utf-8;profile="A;B""#
        );
    }
}

#[test]
fn non_xml_whitespace_before_html_keeps_the_xml_default() {
    let rep = result("", "<xsl:text>&#160;</xsl:text><html/>", None);
    assert_eq!(rep.repr_type.media_type, "application/xml");
    assert_eq!(rep.bytes, "\u{a0}<html/>".as_bytes());
}

#[test]
fn an_as_argument_that_is_not_utf8_is_refused() {
    let stylesheet = br#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template match="/"><out/></xsl:template></xsl:stylesheet>"#;
    let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(stylesheet.to_vec()))
        .with_arg("as", ArgRef::Inline(vec![0xff]));
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let result = block_on(kernel.issue(request, &Capability::root()));
    assert!(
        matches!(&result, Err(ikigai_core::Error::InvalidArgument { name, .. })
        if name == "as"),
        "{result:?}"
    );
}
