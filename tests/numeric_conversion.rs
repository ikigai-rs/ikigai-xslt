//! XPath 1.0 numeric conversion regressions (ledger #2487).
use ikigai_xslt::transform_xml;

fn evaluate(src: &str, expression: &str) -> String {
    let stylesheet = format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:output method="text"/><xsl:template match="/"><xsl:value-of select="{expression}"/></xsl:template></xsl:stylesheet>"#
    );
    transform_xml(src, &stylesheet, true).unwrap_or_else(|error| panic!("{expression}: {error}"))
}

#[test]
fn reported_node_set_number_and_arithmetic() {
    let src = r#"<r><b n="x"><l d="t">12.5</l></b></r>"#;
    assert_eq!(evaluate(src, "concat('[', string(/r/b/l), '][', number(/r/b/l), '][', number(string(/r/b/l)), '][', /r/b/l + 1, ']')"), "[12.5][12.5][12.5][13.5]");
}

#[test]
fn string_length_results_are_numeric_operands() {
    for (expression, expected) in [
        ("string-length('abc') - 1", "2"),
        ("string-length('abc') + 0", "3"),
        ("string-length('abc') * 2", "6"),
        ("string-length('abc') div 3", "1"),
        ("1 + string-length('abc')", "4"),
    ] {
        assert_eq!(evaluate("<r/>", expression), expected, "{expression}");
    }
}

#[test]
fn numeric_operands_use_the_first_node_and_handle_empty_and_invalid_values() {
    let src = "<r n='12.5'><v> 12.5 </v><v>99</v><bad>x</bad><empty/></r>";
    for (expression, expected) in [
        ("number(/r/v)", "12.5"),
        ("number(/r/@n)", "12.5"),
        ("number(/r/v/text())", "12.5"),
        ("number(/r/missing)", "NaN"),
        ("number(/r/empty)", "NaN"),
        ("number(/r/bad)", "NaN"),
        ("/r/v + 1", "13.5"),
        ("1 + /r/v", "13.5"),
        ("/r/v - 2", "10.5"),
        ("/r/v * 2", "25"),
        ("/r/v div 2", "6.25"),
        ("/r/v mod 2", "0.5"),
        ("/r/v + 2 * 3", "18.5"),
        ("(/r/v + 2) * 3", "43.5"),
        ("20 - /r/v - 2", "5.5"),
        ("/r/missing + 1", "NaN"),
        ("1 + /r/bad", "NaN"),
        ("number('128.1355')", "128.1355"),
        ("number(' 12.5 ')", "12.5"),
        ("number('1 2')", "NaN"),
        ("number(string(/r/v[1]))", "12.5"),
        ("count(/r/v) + 1", "3"),
        ("number(/r/v | /r/@n)", "12.5"),
        ("substring('abc', string-length('abc') - 1)", "bc"),
    ] {
        assert_eq!(evaluate(src, expression), expected, "{expression}");
    }
}

#[test]
fn numeric_paths_keep_namespaces_axes_and_predicates() {
    let src = "<r xmlns:p='urn:p'><p:v n='4'>12.5</p:v><p:v n='9'>99</p:v></r>";
    let stylesheet = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:p="urn:p"><xsl:output method="text"/><xsl:template match="/"><xsl:for-each select="r/p:v[@n + 0 = 4]"><xsl:value-of select="number() + number(@*) + number(self::p:v)"/></xsl:for-each></xsl:template></xsl:stylesheet>"#;
    assert_eq!(transform_xml(src, stylesheet, true).unwrap(), "29");
}

#[test]
fn arithmetic_reaches_variables_tests_and_chart_attribute_value_templates() {
    let stylesheet = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template match="/"><xsl:variable name="v" select="/r/v"/><xsl:if test="$v + 1 = 13.5"><circle cx="{$v + 1}" cy="{string-length('abc') * 2}" r="{number($v)}" label="literal + 1 {$v + 2}"/></xsl:if></xsl:template></xsl:stylesheet>"#;
    let result = transform_xml("<r><v>12.5</v></r>", stylesheet, false).unwrap();
    for attribute in [
        "cx='13.5'",
        "cy='6'",
        "r='12.5'",
        "label='literal + 1 14.5'",
    ] {
        assert!(result.contains(attribute), "{result}: missing {attribute}");
    }
}

#[test]
fn operators_inside_names_and_strings_remain_text() {
    let src = "<r><number>12.5</number><a-b>4</a-b><div>2</div></r>";
    for (expression, expected) in [
        ("/r/a-b + /r/div", "6"),
        ("number(/r/number)", "12.5"),
        (
            "concat('number(/r/v) + 1', /r/a-b + 1)",
            "number(/r/v) + 15",
        ),
        ("string-length('é') + 1", "2"),
    ] {
        assert_eq!(evaluate(src, expression), expected, "{expression}");
    }
}

#[test]
fn namespaced_attributes_are_numeric() {
    let src = "<r xmlns:p='urn:p' p:n='12.5'/>";
    let stylesheet = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:p="urn:p"><xsl:template match="/"><p:out p:n="{/r/@p:* + 1}"/></xsl:template></xsl:stylesheet>"#;
    let result = transform_xml(src, stylesheet, false).unwrap();
    assert!(result.contains("p:n='13.5'"), "{result}");
}

#[test]
fn arithmetic_preserves_floating_point_signed_zero() {
    let src = "<r><v>-1</v></r>";
    assert_eq!(evaluate(src, "1 div (0 div /r/v)"), "-inf");
}
