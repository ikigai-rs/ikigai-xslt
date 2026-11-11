//! Regression cases reproduced during the subset-analysis audit (ledger #3412).
use ikigai_xslt::{transform_xml, transform_xml_with_modules};
use std::collections::BTreeMap;

const XSL: &str = "http://www.w3.org/1999/XSL/Transform";

fn templates(body: &str) -> String {
    format!("<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0'><xsl:output method='text'/>{body}</xsl:stylesheet>")
}

fn style(body: &str) -> String {
    templates(&format!("<xsl:template match='/'>{body}</xsl:template>"))
}

fn assert_refused(sheet: &str) {
    let error = transform_xml("<doc/>", sheet, true).unwrap_err();
    assert!(
        error.starts_with("invalid argument `stylesheet`: refused:"),
        "{error}"
    );
}

#[test]
fn copied_booleans_construct_text_instead_of_leaking_into_variable_content() {
    for (body, expected) in [
        ("<xsl:copy-of select='true()'/><xsl:copy-of select='false()'/>", "truefalse"),
        ("<xsl:variable name='v'><xsl:copy-of select='true()'/></xsl:variable><xsl:value-of select='$v'/>", "true"),
        ("<xsl:variable name='v'><xsl:choose><xsl:when test='doc'><xsl:copy-of select='false()'/></xsl:when><xsl:otherwise>fallback</xsl:otherwise></xsl:choose></xsl:variable><xsl:variable name='alias' select='$v'/><xsl:value-of select='string($alias)'/>", "false"),
        // A nonempty result tree fragment has a true boolean value, even if its text is false.
        ("<xsl:variable name='v'><xsl:copy-of select='false()'/></xsl:variable><xsl:value-of select='boolean($v)'/>", "true"),
    ] {
        assert_eq!(transform_xml("<doc/>", &style(body), true).unwrap(), expected, "{body}");
    }
}

#[test]
fn copied_boolean_parameter_content_and_defaults_are_text() {
    for caller in [
        "<xsl:call-template name='show'/>",
        "<xsl:call-template name='show'><xsl:with-param name='p'><xsl:copy-of select='false()'/></xsl:with-param></xsl:call-template>",
    ] {
        let sheet = templates(&format!(
            "<xsl:template match='/'>{caller}</xsl:template><xsl:template name='show'><xsl:param name='p'><xsl:copy-of select='false()'/></xsl:param><xsl:value-of select='$p'/></xsl:template>"
        ));
        assert_eq!(transform_xml("<doc/>", &sheet, true).unwrap(), "false");
    }
    let sheet = templates("<xsl:param name='p'><xsl:copy-of select='true()'/></xsl:param><xsl:template match='/'><xsl:value-of select='$p'/></xsl:template>");
    assert_eq!(transform_xml("<doc/>", &sheet, true).unwrap(), "true");
}

#[test]
fn imported_copied_boolean_defaults_are_adapted() {
    let module = templates("<xsl:template name='show'><xsl:param name='p'><xsl:copy-of select='false()'/></xsl:param><xsl:value-of select='$p'/></xsl:template>");
    let sheet = format!("<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0'><xsl:import href='urn:module'/><xsl:output method='text'/><xsl:template match='/'><xsl:call-template name='show'/></xsl:template></xsl:stylesheet>");
    let modules = BTreeMap::from([("urn:module".into(), module)]);
    assert_eq!(
        transform_xml_with_modules("<doc/>", &sheet, &modules, true).unwrap(),
        "false"
    );
}

#[test]
fn ambiguous_copy_of_parameters_are_refused() {
    let sheet = templates("<xsl:template match='/'><xsl:call-template name='show'><xsl:with-param name='p' select='true()'/></xsl:call-template></xsl:template><xsl:template name='show'><xsl:param name='p'/><xsl:copy-of select='$p'/></xsl:template>");
    assert_refused(&sheet);
}

#[test]
fn atomic_sequences_cannot_leak_boolean_values_into_content_declarations() {
    assert_refused(&style("<xsl:variable name='v'><xsl:sequence select='true()'/></xsl:variable><xsl:value-of select='$v'/>"));
}

#[test]
fn namespaced_core_function_spellings_are_unknown_too() {
    for select in [
        "ex:string(true())",
        "string(ex:string(true()))",
        "ex:true()",
    ] {
        let sheet = style(&format!(
            "<xsl:value-of xmlns:ex='urn:extension' select='{select}'/>"
        ));
        assert_refused(&sheet);
    }
    let sheet = style("<xsl:variable xmlns:ex='urn:extension' name='v' select='ex:string(true())'/><xsl:value-of select='$v'/>");
    assert_refused(&sheet);
    // Function spellings inside literals and ordinary QName/axis steps are not calls.
    assert_eq!(
        transform_xml(
            "<doc/>",
            &style(
                "<xsl:value-of select=\"'ex:string(true())'\"/><xsl:value-of select='child::doc'/>"
            ),
            true
        )
        .unwrap(),
        "ex:string(true())"
    );
}

#[test]
fn xpath_bindings_cannot_bypass_boolean_result_or_conversion_analysis() {
    for select in [
        "for $v in doc return true()",
        "let $v := true() return $v",
        "string(for $v in doc return true())",
        "number(let $v := true() return $v)",
        "(for $v in doc return true()) + 1",
    ] {
        assert_refused(&style(&format!("<xsl:value-of select='{select}'/>")));
    }
    for select in ["$p", "string($p)", "number($p)"] {
        let sheet = templates(&format!("<xsl:template match='/'><xsl:call-template name='show'/></xsl:template><xsl:template name='show'><xsl:param name='p' select='for $v in doc return true()'/><xsl:value-of select='{select}'/></xsl:template>"));
        assert_refused(&sheet);
    }
}

#[test]
fn conditional_boolean_conversions_are_adapted() {
    for (select, expected) in [
        ("string(if (true()) then true() else false())", "true"),
        ("string(if (false()) then true() else false())", "false"),
        ("number(if (true()) then false() else true())", "0"),
        ("(if (true()) then true() else false()) + 1", "2"),
        (
            "string(if (true()) then (if (false()) then true() else false()) else true())",
            "false",
        ),
        ("string(if (true()) then 42 else 0)", "42"),
    ] {
        assert_eq!(
            transform_xml(
                "<doc/>",
                &style(&format!("<xsl:value-of select='{select}'/>")),
                true
            )
            .unwrap(),
            expected,
            "{select}"
        );
    }
    for select in [
        "string(if (true()) then true() else 0)",
        "number(if (true()) then 0 else false())",
    ] {
        assert_refused(&style(&format!("<xsl:value-of select='{select}'/>")));
    }
}
