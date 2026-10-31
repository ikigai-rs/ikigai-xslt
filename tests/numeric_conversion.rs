//! XPath 1.0 numeric conversion regressions (ledger #2487 and #2668).
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

// Each reported row is independent so the baseline run reproduces all six (#2668).
macro_rules! reported_boolean_numeric_case {
    ($name:ident, $expression:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(evaluate("<r><v>3</v></r>", $expression), $expected);
        }
    };
}
reported_boolean_numeric_case!(boolean_number_true, "number(true())", "1");
reported_boolean_numeric_case!(boolean_number_false, "number(false())", "0");
reported_boolean_numeric_case!(boolean_addition, "true() + 1", "2");
reported_boolean_numeric_case!(boolean_comparison_number, "number(/r/v &gt; 2) * 5", "5");
reported_boolean_numeric_case!(
    boolean_conditional_substring,
    "concat('[', substring('yes', 1, number(/r/v &gt; 2) * 3), ']')",
    "[yes]"
);
reported_boolean_numeric_case!(
    large_literal_addition,
    "100000000000000000000 + 1",
    "100000000000000000000"
);

#[test]
fn boolean_conversion_preserves_types_and_false_conditions() {
    for (expression, expected) in [
        ("string(true())", "true"),
        ("string(false())", "false"),
        ("concat('[', string(/r/v &gt; 2), ']')", "[true]"),
        ("number('true')", "NaN"),
        ("number('false')", "NaN"),
        ("number(string(true()))", "NaN"),
        ("number(string(false()))", "NaN"),
        ("false() + 1", "1"),
        ("true() - false()", "1"),
        ("true() * 5", "5"),
        ("true() div 2", "0.5"),
        ("true() mod 2", "1"),
        ("-true()", "-1"),
        ("1 div (-false())", "-inf"),
        ("1 div (+ (0 div /r/negative))", "-inf"),
        ("number(not(/r/v))", "0"),
        ("number(contains('abc', 'b'))", "1"),
        ("number(starts-with('abc', 'b'))", "0"),
        ("number((true() and false()) or true())", "1"),
        (
            "concat('[', substring('yes', 1, number(/r/v &lt; 2) * 3), ']')",
            "[]",
        ),
        ("100000000000000000000", "100000000000000000000"),
        ("-100000000000000000000 + 1", "-100000000000000000000"),
        (
            "concat('100000000000000000000', ' + true()')",
            "100000000000000000000 + true()",
        ),
    ] {
        assert_eq!(
            evaluate("<r><v>3</v><negative>-1</negative></r>", expression),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn relational_conditions_compare_all_numeric_node_pairs_in_order() {
    let src = "<r><v>1</v><v>3</v><w>2</w><w>4</w><bad>x</bad></r>";
    for (expression, expected) in [
        ("number(/r/v &gt; 2)", "1"),
        ("number(/r/v &lt; 0)", "0"),
        ("number(2 &lt; /r/v)", "1"),
        ("number(/r/v &gt; /r/w)", "1"),
        ("number(/r/v &gt;= 3)", "1"),
        ("number(/r/v &lt;= 1)", "1"),
        ("number(/r/missing &gt; 0)", "0"),
        ("number(0 &gt; /r/missing)", "0"),
        ("number(/r/bad &gt; 0)", "0"),
        ("number('10' &lt; '2')", "0"),
        ("number(true() &gt; false())", "1"),
    ] {
        assert_eq!(evaluate(src, expression), expected, "{expression}");
    }
}

#[test]
fn boolean_variables_parameters_predicates_and_avts_are_adapted() {
    let stylesheet = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template match="/"><xsl:variable name="__ikigai_numeric_item" select="2"/><xsl:variable name="b" select="/r/v &gt; $__ikigai_numeric_item"/><xsl:variable name="alias" select="$b"/><out n="{number($alias) * 5}" s="{string($alias)}"><xsl:for-each select="r/v[number(. &gt; 2) = 1]"><xsl:value-of select="number($b) + 1"/></xsl:for-each><xsl:call-template name="local"><xsl:with-param name="flag" select="$alias"/></xsl:call-template></out></xsl:template><xsl:template name="local"><xsl:param name="flag" select="false()"/><xsl:value-of select="concat('[', string($flag), '][', number($flag), ']')"/></xsl:template></xsl:stylesheet>"#;
    let result = transform_xml("<r><v>1</v><v>3</v></r>", stylesheet, false).unwrap();
    assert!(result.contains("n='5'"), "{result}");
    assert!(result.contains("s='true'"), "{result}");
    assert!(result.ends_with(">2[true][1]</out>"), "{result}");
}

#[test]
fn ambiguous_parameter_conversions_are_refused_explicitly() {
    for conversion in ["number($flag)", "$flag + 1", "string($flag)"] {
        let stylesheet = format!(
            r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:param name="flag" select="true()"/><xsl:template match="/"><xsl:value-of select="{conversion}"/></xsl:template></xsl:stylesheet>"#
        );
        let error = transform_xml("<r/>", &stylesheet, true).unwrap_err();
        assert!(
            error.starts_with("invalid argument `stylesheet`:"),
            "{error}"
        );
        assert!(
            error.contains("boolean") && error.contains("xsl:choose"),
            "{error}"
        );
    }
}
