//! Union match templates are split into one rule per alternative; every alternative
//! resolves its prefixes against the namespaces in scope at its own template (ledger #3396).

use std::collections::BTreeMap;

const XSL: &str = "http://www.w3.org/1999/XSL/Transform";

fn transform(src: &str, stylesheet: &str) -> Result<String, String> {
    ikigai_xslt::transform_xml(src, stylesheet, true)
}

#[test]
fn a_union_of_two_prefixed_names_compiles_and_matches_both() {
    let style = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0' xmlns:foaf='http://xmlns.com/foaf/0.1/'>\
         <xsl:template match='/'><xsl:apply-templates select='*/*' mode='m'/></xsl:template>\
         <xsl:template match='foaf:homepage | foaf:weblog' mode='m'>[<xsl:value-of select='local-name()'/>]</xsl:template>\
         <xsl:template match='*' mode='m'>other</xsl:template>\
         </xsl:stylesheet>"
    );
    let src = "<doc xmlns:f='http://xmlns.com/foaf/0.1/'><f:homepage/><f:weblog/><f:name/></doc>";
    assert_eq!(transform(src, &style).unwrap(), "[homepage][weblog]other");
}

#[test]
fn every_alternative_resolves_its_own_prefix() {
    let style = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0' xmlns:a='urn:a' xmlns:b='urn:b'>\
         <xsl:template match='/'><xsl:apply-templates select='*/*'/></xsl:template>\
         <xsl:template match='plain | a:x | b:y'>[<xsl:value-of select='local-name()'/>]</xsl:template>\
         <xsl:template match='*'>other</xsl:template>\
         </xsl:stylesheet>"
    );
    let src = "<doc xmlns:p='urn:a' xmlns:q='urn:b'><plain/><p:x/><q:y/><p:y/></doc>";
    assert_eq!(transform(src, &style).unwrap(), "[plain][x][y]other");
}

#[test]
fn a_prefix_declared_on_the_template_itself_reaches_every_alternative() {
    let style = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0'>\
         <xsl:template match='/'><xsl:apply-templates select='*/*'/></xsl:template>\
         <xsl:template xmlns:t='urn:t' match='t:a | t:b'>[<xsl:value-of select='local-name()'/>]</xsl:template>\
         <xsl:template match='*'>other</xsl:template>\
         </xsl:stylesheet>"
    );
    let src = "<doc xmlns:z='urn:t'><z:a/><z:b/><c/></doc>";
    assert_eq!(transform(src, &style).unwrap(), "[a][b]other");
}

#[test]
fn an_imported_union_uses_its_own_modules_binding_for_a_shared_prefix() {
    // The importer binds `p` to urn:main, the module binds `p` to urn:module. The fallback
    // lives in the module: in the importer it would outrank every imported rule.
    let module = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0' xmlns:p='urn:module'>\
         <xsl:template match='p:a | p:b'>module:<xsl:value-of select='local-name()'/>;</xsl:template>\
         <xsl:template match='*' priority='-1'>other;</xsl:template>\
         </xsl:stylesheet>"
    );
    let main = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0' xmlns:p='urn:main'>\
         <xsl:import href='urn:file:module'/>\
         <xsl:template match='/'><xsl:apply-templates select='*/*'/></xsl:template>\
         <xsl:template match='p:a | p:b'>main:<xsl:value-of select='local-name()'/>;</xsl:template>\
         </xsl:stylesheet>"
    );
    let modules = BTreeMap::from([("urn:file:module".to_string(), module)]);
    let src = "<doc xmlns:m='urn:main' xmlns:o='urn:module'>\
               <m:a/><m:b/><o:a/><o:b/><a/></doc>";
    assert_eq!(
        ikigai_xslt::transform_xml_with_modules(src, &main, &modules, true).unwrap(),
        "main:a;main:b;module:a;module:b;other;"
    );
}

#[test]
fn split_prefixed_alternatives_keep_their_default_priorities() {
    // XSLT 1.0 5.5: `p:a` is 0, `p:*` is -0.25, `p:a/p:b` is 0.5, each as its own rule.
    let style = format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0' xmlns:p='urn:p'>\
         <xsl:template match='/'><xsl:apply-templates select='//p:*'/></xsl:template>\
         <xsl:template match='p:x | p:* | p:a/p:b'>union;</xsl:template>\
         <xsl:template match='p:*' priority='-0.1'>wild;</xsl:template>\
         <xsl:template match='p:b' priority='0.25'>name;</xsl:template>\
         </xsl:stylesheet>"
    );
    // p:a: only `p:*` (-0.25) of the union matches, losing to -0.1.
    // p:b under p:a: `p:a/p:b` (0.5) beats 0.25.
    // p:x: `p:x` (0) beats -0.1.
    let src = "<doc xmlns:p='urn:p'><p:a><p:b/></p:a><p:x/></doc>";
    assert_eq!(transform(src, &style).unwrap(), "wild;union;union;");
}

#[test]
fn the_foaf_shape_six_way_prefixed_union_under_a_mode() {
    let style = include_str!("fixtures/foaf-union.xsl");
    let src = include_str!("fixtures/foaf-union.rdf");
    assert_eq!(
        transform(src, style).unwrap(),
        include_str!("fixtures/foaf-union.expected.txt").trim_end()
    );
}
