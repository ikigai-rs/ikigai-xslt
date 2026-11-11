//! Synthetic XSLT 1.0 / XPath 1.0 audit cases (no external fixtures).
use ikigai_xslt::transform_xml;

fn sheet(top: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:p="urn:p" xmlns:q="urn:q"><xsl:output method="xml" omit-xml-declaration="yes"/>{top}</xsl:stylesheet>"#
    )
}
fn run(src: &str, top: &str) -> Result<String, String> {
    transform_xml(src, &sheet(top), false)
}
fn correct_or_refused(src: &str, top: &str, expected: &str, construct: &str) {
    match run(src, top) {
        Ok(out) => assert_eq!(out, expected, "{construct}"),
        Err(error) => assert!(
            error.starts_with("invalid argument `stylesheet`:") && error.contains(construct),
            "{construct}: {error}"
        ),
    }
}
#[test]
fn skipped_areas() {
    let src = "<doc><item k='1'>one</item><item k='2'>two</item></doc>";
    correct_or_refused(
        src,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item[@k='2']">hit</xsl:template>"#,
        "<out>onehit</out>",
        "a predicate in a match pattern",
    );
    correct_or_refused(
        src,
        r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:value-of select="position()"/><xsl:value-of select="last()"/></xsl:for-each></out></xsl:template>"#,
        "<out>1222</out>",
        "position() or last()",
    );
    correct_or_refused(
        src,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="//item">hit</xsl:template>"#,
        "<out>hithit</out>",
        "leading //",
    );
    for (expr, expected) in [
        ("'10' &lt; '2'", "false"),
        ("-(-2)", "2"),
        ("100000000000000000000", "100000000000000000000"),
    ] {
        assert_eq!(run(src, &format!(r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#)).unwrap(), format!("<out>{expected}</out>"));
    }
}
#[test]
fn multiple_nodes_become_the_first_string() {
    let cases = [
        ("doc/item", "one"),
        ("string(doc/item)", "one"),
        ("string-length(doc/item)", "3"),
        ("concat(doc/item, '!')", "one!"),
    ];
    let actual: Vec<_> = cases.iter().map(|(expr, _)| run("<doc><item>one</item><item>two</item></doc>", &format!(r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#)).unwrap()).collect();
    let expected: Vec<_> = cases
        .iter()
        .map(|(_, answer)| format!("<out>{answer}</out>"))
        .collect();
    assert_eq!(actual, expected);
}
#[test]
fn built_in_templates_keep_the_mode() {
    assert_eq!(run("<doc><item>A</item><!--ignored--><?pi ignored?></doc>", r#"<xsl:template match="/"><out><xsl:apply-templates select="doc" mode="m"/></out></xsl:template><xsl:template match="item" mode="m">M</xsl:template><xsl:template match="item">D</xsl:template>"#).unwrap(), "<out>M</out>");
}
#[test]
fn escaped_avt_braces() {
    correct_or_refused(
        "<doc/>",
        r#"<xsl:template match="/"><out a="{{literal}}" b="{{{1}}}"/></xsl:template>"#,
        "<out a='{literal}' b='{1}'/>",
        "escaped braces",
    );
}
#[test]
fn text_is_escaped_once() {
    for text in [
        r#"<xsl:text>&lt;&amp;</xsl:text>"#,
        r#"<xsl:text disable-output-escaping="no">&lt;&amp;</xsl:text>"#,
        r#"<xsl:value-of select="'&lt;&amp;'"/>"#,
    ] {
        assert_eq!(
            run(
                "<doc/>",
                &format!(r#"<xsl:template match="/"><out>{text}</out></xsl:template>"#)
            )
            .unwrap(),
            "<out>&lt;&amp;</out>",
            "{text}"
        );
    }
}
#[test]
fn namespace_alias_is_applied_or_refused() {
    correct_or_refused(
        "<doc/>",
        r#"<xsl:namespace-alias stylesheet-prefix="p" result-prefix="q"/><xsl:template match="/"><p:out/></xsl:template>"#,
        "<q:out xmlns:q='urn:q'/>",
        "namespace-alias",
    );
}
#[test]
fn excluded_unused_namespace_is_absent() {
    assert_eq!(
        run(
            "<doc/>",
            r#"<xsl:template match="/"><out xsl:exclude-result-prefixes="p q"/></xsl:template>"#
        )
        .unwrap(),
        "<out/>"
    );
}
#[test]
fn xml_declaration_is_honored_or_refused() {
    let style = sheet(r#"<xsl:template match="/"><out/></xsl:template>"#).replace(
        "omit-xml-declaration=\"yes\"",
        "omit-xml-declaration=\"no\"",
    );
    match transform_xml("<doc/>", &style, false) {
        Ok(out) => assert!(out.starts_with("<?xml version="), "{out}"),
        Err(e) => assert!(
            e.starts_with("invalid argument `stylesheet`:") && e.contains("omit-xml-declaration"),
            "{e}"
        ),
    }
}
#[test]
fn template_priorities_and_ties() {
    for (templates, expected) in [
        (
            r#"<xsl:template match="*">wild</xsl:template><xsl:template match="item">name</xsl:template>"#,
            "name",
        ),
        (
            r#"<xsl:template match="doc/item">path</xsl:template><xsl:template match="item">name</xsl:template>"#,
            "path",
        ),
        (
            r#"<xsl:template match="item" priority="2">high</xsl:template><xsl:template match="item" priority="1">low</xsl:template>"#,
            "high",
        ),
        (
            r#"<xsl:template match="item">first</xsl:template><xsl:template match="item">last</xsl:template>"#,
            "last",
        ),
    ] {
        assert_eq!(run("<doc><item/></doc>", &format!(r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template>{templates}"#)).unwrap(), format!("<out>{expected}</out>"));
    }
}
#[test]
fn built_in_node_types() {
    assert_eq!(run("<doc a='A'>T<!--C--><?pi P?><child>B</child></doc>", r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/@* | doc/node()"/></out></xsl:template>"#).unwrap(), "<out>ATB</out>");
}
#[test]
fn text_method_preserves_characters() {
    let style = sheet(r#"<xsl:template match="/"><out><xsl:text>&lt;&amp;</xsl:text><xsl:value-of select="'&lt;&amp;'"/></out></xsl:template>"#).replace("method=\"xml\"", "method=\"text\"");
    assert_eq!(transform_xml("<doc/>", &style, true).unwrap(), "<&<&");
}
#[test]
fn unsupported_encoding_is_refused() {
    let style = sheet(r#"<xsl:template match="/"><out>é</out></xsl:template>"#)
        .replace("method=\"xml\"", "method=\"xml\" encoding=\"UTF-16\"");
    let result = transform_xml("<doc/>", &style, false);
    assert!(
        matches!(result, Err(ref e) if e.starts_with("invalid argument `stylesheet`:") && e.contains("encoding")),
        "{result:?}"
    );
}
#[test]
fn recursion_is_a_typed_refusal() {
    let result = run(
        "<doc/>",
        r#"<xsl:template match="/"><out>partial<xsl:call-template name="loop"/></out></xsl:template><xsl:template name="loop"><xsl:call-template name="loop"/></xsl:template>"#,
    );
    assert!(
        matches!(result, Err(ref e) if e.starts_with("invalid argument `stylesheet`:") || e.starts_with("timeout:")),
        "{result:?}"
    );
}
#[test]
fn html_boolean_attributes_preserve_html_semantics() {
    let style = sheet(r#"<xsl:template match="/"><input checked="checked"/></xsl:template>"#)
        .replace("method=\"xml\"", "method=\"html\"");
    match transform_xml("<doc/>", &style, false) {
        Ok(out) => assert_eq!(
            out, "<input checked='checked'/>",
            "equivalent HTML boolean syntax"
        ),
        Err(e) => assert!(
            e.starts_with("invalid argument `stylesheet`:") && e.contains("boolean"),
            "{e}"
        ),
    }
}
#[test]
fn about_shape_is_byte_identical() {
    assert_eq!(
        transform_xml(
            include_str!("fixtures/foaf-last-segment.rdf"),
            include_str!("fixtures/foaf-last-segment.xsl"),
            false
        )
        .unwrap(),
        include_str!("fixtures/foaf-last-segment.expected.html")
    );
}

#[test]
fn priorities_include_namespace_wildcards_and_pi_targets() {
    assert_eq!(run("<doc xmlns:p='urn:p'><p:item/></doc>", r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/node()"/></out></xsl:template><xsl:template match="p:*">prefix</xsl:template><xsl:template match="*">wild</xsl:template>"#).unwrap(), "<out>prefix</out>");
    correct_or_refused(
        "<doc><?target x?></doc>",
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/node()"/></out></xsl:template><xsl:template match="processing-instruction('target')">target</xsl:template><xsl:template match="processing-instruction()">pi</xsl:template>"#,
        "<out>target</out>",
        "processing-instruction with a target",
    );
}

#[test]
fn exclusion_keeps_a_namespace_used_in_a_result_name() {
    let style = sheet(
        r#"<xsl:template match="/"><p:out xsl:exclude-result-prefixes="p q"/></xsl:template>"#,
    );
    let expected = "<p:out xmlns:p='urn:p'/>";
    let compiled = ikigai_xslt::CompiledStylesheet::compile(&style).unwrap();
    for _ in 0..5 {
        assert_eq!(compiled.transform("<doc/>", false).unwrap(), expected);
    }
}

#[test]
fn refusal_types_survive_the_endpoint() {
    use futures::executor::block_on;
    use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
    use std::sync::Arc;
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for (top, construct) in [
        (
            r#"<xsl:namespace-alias stylesheet-prefix="p" result-prefix="q"/>"#,
            "namespace-alias",
        ),
        (
            r#"<xsl:template match="/"><out a="{{literal}}"/></xsl:template>"#,
            "escaped braces",
        ),
        (
            r#"<xsl:template match="/"><out>partial<xsl:call-template name="loop"/></out></xsl:template><xsl:template name="loop"><xsl:call-template name="loop"/></xsl:template>"#,
            "evaluation depth",
        ),
    ] {
        let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
            .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
            .with_arg("stylesheet", ArgRef::Inline(sheet(top).into_bytes()));
        let result = block_on(kernel.issue(req, &Capability::root()));
        assert!(
            matches!(result, Err(Error::InvalidArgument { ref name, ref detail }) if name == "stylesheet" && detail.contains(construct)),
            "{construct}: {result:?}"
        );
    }
}

#[test]
fn inline_html_code_is_raw_or_refused() {
    for element in ["script", "style"] {
        let style = sheet(&format!(r#"<xsl:template match="/"><{element}>a &lt; b &amp;&amp; c = 'd'</{element}></xsl:template>"#)).replace("method=\"xml\"", "method=\"html\"");
        match transform_xml("<doc/>", &style, false) {
            Ok(out) => assert_eq!(out, format!("<{element}>a < b && c = 'd'</{element}>")),
            Err(e) => assert!(
                e.starts_with("invalid argument `stylesheet`:") && e.contains(element),
                "{e}"
            ),
        }
    }
}
