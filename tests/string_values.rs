//! XPath 1.0 string conversion uses the first node, including function arguments.
use ikigai_xslt::transform_xml;

#[test]
fn string_functions_convert_first_node_and_empty_sets() {
    let cases = [
        ("doc/item", "one"),
        ("string(doc/item)", "one"),
        ("string-length(doc/item)", "3"),
        ("concat(doc/item, '!')", "one!"),
        ("starts-with(doc/item, 'onetwo')", "false"),
        ("contains(doc/item, 'two')", "false"),
        ("substring-before(doc/item, 'two')", ""),
        ("substring-after(doc/item, 'e')", ""),
        ("substring(doc/item, 2)", "ne"),
        ("normalize-space(doc/item)", "one"),
        ("translate(doc/item, 'o', 'O')", "One"),
        ("concat(doc/missing, '!')", "!"),
        ("string-length(doc/missing)", "0"),
        ("string(true())", "true"),
        ("string(false())", "false"),
        ("concat('one', 'two')", "onetwo"),
    ];
    for (expression, expected) in cases {
        let stylesheet = format!(
            r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:output method="xml"/><xsl:template match="/"><out><xsl:value-of select="{expression}"/></out></xsl:template></xsl:stylesheet>"#
        );
        assert_eq!(
            transform_xml(
                "<doc><item>one</item><item>two</item></doc>",
                &stylesheet,
                false
            )
            .unwrap(),
            format!("<out>{expected}</out>"),
            "{expression}"
        );
    }
}

#[test]
fn copied_nodes_stay_a_node_set() {
    let style = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template match="/"><out><xsl:copy-of select="doc/item"/></out></xsl:template></xsl:stylesheet>"#;
    assert_eq!(
        transform_xml("<doc><item>one</item><item>two</item></doc>", style, false).unwrap(),
        "<out><item>one</item><item>two</item></out>"
    );
}
