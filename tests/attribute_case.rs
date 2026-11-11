//! Prefixed attribute values keep their case (ledger #3342, after #2519).
//!
//! xrust 2.3.0's XML parser matched on `value.to_lowercase()` to compare `xml:space`, and then
//! built `xml:id` and every other prefixed attribute from that lowercased binding. This crate
//! carries xrust as `ikigai-xrust` (`crates/xrust`) with that fix (among others, ledger #3343), so the values below
//! come through as written, in the source and in the stylesheet alike.

use ikigai_xslt::transform_xml;
use xrust::item::Node;
use xrust::parser::xml::parse;
use xrust::parser::ParseError;
use xrust::trees::smite::RNode;

/// RDF/XML with capitalized values in the three attributes the FOAF face reads (#3341).
const RDF: &str = r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <foaf:Person rdf:about="https://Example.org/People/Alice#Me" dc:title="Dr. Alice Example">
    <foaf:homepage rdf:resource="https://Example.org/About-Alice"/>
  </foaf:Person>
</rdf:RDF>"#;

const RDF_STYLE: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:foaf="http://xmlns.com/foaf/0.1/">
<xsl:output method="xml"/>
<xsl:template match="/"><out><xsl:for-each select="rdf:RDF/foaf:Person"><p about="{@rdf:about}" title="{@dc:title}" home="{foaf:homepage/@rdf:resource}"/></xsl:for-each></out></xsl:template>
</xsl:stylesheet>"#;

fn parse_doc(text: &str) -> Result<RNode, xrust::xdmerror::Error> {
    let doc = RNode::new_document();
    parse(
        doc.clone(),
        text,
        Some(|_: &_| {
            Err(ParseError::MissingNameSpace {
                prefix: None,
                nsuri: None,
                message: None,
            })
        }),
    )
}

fn parsed(text: &str) -> RNode {
    parse_doc(text).expect("well-formed")
}

/// The parser itself, with no transform in between: what xrust 2.3.0 got wrong.
#[test]
fn the_parser_keeps_prefixed_attribute_values_as_written() {
    let doc = parsed(RDF);
    let xml = doc.to_xml();
    for value in [
        "https://Example.org/People/Alice#Me",
        "Dr. Alice Example",
        "https://Example.org/About-Alice",
    ] {
        assert!(xml.contains(value), "`{value}` survives parsing: {xml}");
    }
    let doc = parsed(r#"<doc xml:id="Upper-Case_ID"/>"#);
    assert!(
        doc.to_xml().contains("Upper-Case_ID"),
        "xml:id keeps its case: {}",
        doc.to_xml()
    );
}

/// The shape that broke the FOAF face: RDF/XML in, values preserved out.
#[test]
fn rdf_xml_with_capitalized_values_transforms_with_values_preserved() {
    assert_eq!(
        transform_xml(RDF, RDF_STYLE, false).unwrap(),
        "<out><p about='https://Example.org/People/Alice#Me' home='https://Example.org/About-Alice' title='Dr. Alice Example'/></out>"
    );
}

/// `xml:space` is still compared without regard to case, as xrust 2.3.0 compares it, and an
/// unknown value is still refused by the parser.
#[test]
fn xml_space_is_still_compared_without_case() {
    assert!(parsed(r#"<doc xml:space="Preserve"/>"#)
        .to_xml()
        .contains("xml:space='preserve'"));
    assert!(parse_doc(r#"<doc xml:space="Sometimes"/>"#).is_err());
}
