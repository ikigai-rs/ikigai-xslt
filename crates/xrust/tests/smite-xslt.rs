mod smite;
mod xsltgeneric;

// XSLT tests

#[test]
fn xslt_style_pos() {
    xsltgeneric::style_pos(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_literal_text() {
    xsltgeneric::generic_literal_text(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_sys_prop() {
    xsltgeneric::generic_sys_prop(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_value_of_1() {
    xsltgeneric::generic_value_of_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_value_of_2() {
    xsltgeneric::generic_value_of_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_literal_element() {
    xsltgeneric::generic_literal_element(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_element() {
    xsltgeneric::generic_element(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_apply_templates_1() {
    xsltgeneric::generic_apply_templates_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_apply_templates_2() {
    xsltgeneric::generic_apply_templates_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_apply_templates_mode() {
    xsltgeneric::generic_apply_templates_mode(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_apply_templates_mode_bad_qname() {
    xsltgeneric::generic_apply_templates_mode_bad_qname(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_apply_templates_sort() {
    xsltgeneric::generic_apply_templates_sort(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_comment() {
    xsltgeneric::generic_comment(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_text_output_1() {
    xsltgeneric::text_output_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_pi() {
    xsltgeneric::generic_pi(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_message_1() {
    xsltgeneric::generic_message_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_message_term() {
    xsltgeneric::generic_message_term(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_58() {
    xsltgeneric::generic_issue_58(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_95() {
    xsltgeneric::generic_issue_95(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_callable_named_1() {
    xsltgeneric::generic_callable_named_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_callable_recurse_1() {
    xsltgeneric::callable_recurse_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_callable_posn_1() {
    xsltgeneric::generic_callable_posn_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
#[should_panic]
fn xslt_include() {
    xsltgeneric::generic_include(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_current() {
    xsltgeneric::generic_current(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_key_1() {
    xsltgeneric::generic_key_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_key_2() {
    xsltgeneric::doc_key_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_key_3() {
    xsltgeneric::doc_key_3(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_key_4() {
    xsltgeneric::doc_key_4(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_key_5() {
    xsltgeneric::doc_key_5(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_document_1() {
    xsltgeneric::generic_document_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_number_1() {
    xsltgeneric::generic_number_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_attr_set_1() {
    xsltgeneric::attr_set_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_attr_set_2() {
    xsltgeneric::attr_set_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_attr_set_3() {
    xsltgeneric::attr_set_3(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_feg_starting_with_1() {
    xsltgeneric::feg_starting_with_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_feg_starting_with_2() {
    xsltgeneric::feg_starting_with_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_96_abs() {
    xsltgeneric::issue_96_abs(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_96_rel() {
    xsltgeneric::issue_96_rel(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_96_mixed() {
    xsltgeneric::issue_96_mixed(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_sd_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_126() {
    xsltgeneric::issue_126(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_137_1() {
    xsltgeneric::issue_137_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_issue_137_2() {
    xsltgeneric::issue_137_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_dbk_1() {
    xsltgeneric::dbk_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_md_1() {
    xsltgeneric::md_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_ghissue_147() {
    xsltgeneric::ghissue_147(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_glissue_147_a() {
    xsltgeneric::gl_issue_147_a(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_gl_issue_147_b() {
    xsltgeneric::gl_issue_147_b(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_1() {
    xsltgeneric::conform_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_2() {
    xsltgeneric::conform_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_3() {
    xsltgeneric::conform_3(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_4() {
    xsltgeneric::conform_4(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_5() {
    xsltgeneric::conform_5(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_6() {
    xsltgeneric::conform_6(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_7() {
    xsltgeneric::conform_7(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_conform_8() {
    xsltgeneric::conform_8(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_namespace_1() {
    xsltgeneric::namespace_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_variable_1() {
    xsltgeneric::variable_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_variable_2() {
    xsltgeneric::variable_2(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}
#[test]
fn xslt_avt_1() {
    xsltgeneric::avt_1(
        smite::make_from_str,
        smite::make_from_str_with_ns,
        smite::make_empty_doc_cooked,
    )
    .expect("test failed")
}

/// Transform `src` with `style`, both XML text, as the `xslt` module documentation does,
/// and serialize the result.
fn transform_str(src: &str, style: &str) -> String {
    use xrust::item::{Item, Node, SequenceTrait};
    use xrust::transform::context::StaticContextBuilder;
    use xrust::trees::smite::RNode;
    use xrust::xdmerror::{Error, ErrorKind};
    use xrust::xslt::from_document;

    let srcdoc = smite::make_from_str(src).expect("unable to parse source");
    let styledoc = smite::make_from_str(style).expect("unable to parse stylesheet");
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "no fetcher")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "no parser")))
        .build();
    let mut ctxt = from_document(styledoc, None, smite::make_from_str, |_| {
        Err(Error::new(ErrorKind::NotImplemented, "no modules"))
    })
    .expect("unable to compile stylesheet");
    ctxt.context(vec![Item::Node(srcdoc)], 0);
    ctxt.result_document(RNode::new_document());
    ctxt.evaluate(&mut stctxt)
        .expect("unable to evaluate stylesheet")
        .to_xml()
}

/// `templates` inside an XSLT 1.0 stylesheet with XML output.
fn stylesheet(templates: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:output method="xml"/>{templates}</xsl:stylesheet>"#
    )
}

#[test]
fn xslt_whitespace_only_text_is_preserved() {
    for text in [" ", "  ", "\t\n", "&#x20;"] {
        let style = stylesheet(&format!(
            r#"<xsl:template match="/">
  <out>Meta<xsl:text>{text}</xsl:text>Source</out>
</xsl:template>"#
        ));
        let kept = if text == "&#x20;" { " " } else { text };
        assert_eq!(
            transform_str("<doc/>", &style),
            format!("<out>Meta{kept}Source</out>")
        );
    }
}

#[test]
fn xslt_boolean_string_value() {
    for (expr, answer) in [
        ("1 &gt; 0", "true"),
        ("1 &lt; 0", "false"),
        ("true()", "true"),
        ("not(true())", "false"),
        ("string(true())", "true"),
        ("concat('[', false(), ']')", "[false]"),
    ] {
        let style = stylesheet(&format!(
            r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#
        ));
        assert_eq!(
            transform_str("<doc/>", &style),
            format!("<out>{answer}</out>"),
            "{expr}"
        );
    }
}

#[test]
fn xslt_boolean_number_value() {
    for (expr, answer) in [
        ("number(true())", "1"),
        ("number(false())", "0"),
        ("true() + 1", "2"),
        ("number(1 &gt; 0) * 5", "5"),
    ] {
        let style = stylesheet(&format!(
            r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#
        ));
        assert_eq!(
            transform_str("<doc/>", &style),
            format!("<out>{answer}</out>"),
            "{expr}"
        );
    }
}

#[test]
fn xslt_node_on_the_left_of_a_comparison() {
    for (test, answer) in [
        ("/r/v &gt; 2", "yes"),
        ("/r/v &lt; 2", "no"),
        ("/r/v &gt;= 3", "yes"),
        ("/r/v &lt;= 2", "no"),
        ("2 &lt; /r/v", "yes"),
        ("/r/v = 3", "yes"),
    ] {
        let style = stylesheet(&format!(
            r#"<xsl:template match="/"><out><xsl:choose><xsl:when test="{test}">yes</xsl:when><xsl:otherwise>no</xsl:otherwise></xsl:choose></out></xsl:template>"#
        ));
        assert_eq!(
            transform_str("<r><v>3</v></r>", &style),
            format!("<out>{answer}</out>"),
            "{test}"
        );
    }
}

#[test]
fn xslt_number_of_nodes_and_unsigned_results() {
    let src = r#"<r><b n=" 7 "><l>12.5</l><l>4</l></b><e/><t>x</t></r>"#;
    for (expr, answer) in [
        ("number(/r/b/l)", "12.5"),
        ("/r/b/l + 1", "13.5"),
        ("/r/b/@n * 2", "14"),
        ("number(/r/missing)", "NaN"),
        ("/r/missing + 1", "NaN"),
        ("number(/r/t)", "NaN"),
        ("string-length('abc') - 1", "2"),
        ("string-length('abc') * 2", "6"),
    ] {
        let style = stylesheet(&format!(
            r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#
        ));
        assert_eq!(
            transform_str(src, &style),
            format!("<out>{answer}</out>"),
            "{expr}"
        );
    }
}
#[test]
fn xslt_literal_attribute_namespace_is_declared() {
    use xrust::item::Node;
    let rdf = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    for body in [
        r#"<out><b rdf:datatype="x">1</b></out>"#,
        r#"<out><b xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" rdf:datatype="x">1</b></out>"#,
        r#"<out><b rdf:datatype="x" rdf:about="y" xml:lang="en">1</b></out>"#,
    ] {
        let style = format!(
            r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:rdf="{rdf}"><xsl:template match="/">{body}</xsl:template></xsl:stylesheet>"#
        );
        let result = transform_str("<doc/>", &style);
        assert!(
            result.contains(&format!("xmlns:rdf='{rdf}'")) && result.contains("rdf:datatype='x'"),
            "{body}: {result}"
        );
        let reparsed = smite::make_from_str(&result).expect("the result is namespace-well-formed");
        assert!(reparsed.to_xml().contains("rdf:datatype='x'"), "{result}");
    }
}

#[test]
fn xslt_text_escapes_once_and_honors_disable_output_escaping() {
    for (attribute, expected) in [
        ("", "&lt;b/&gt;&amp;"),
        (" disable-output-escaping='no'", "&lt;b/&gt;&amp;"),
        (" disable-output-escaping='yes'", "<b/>&"),
    ] {
        let style = stylesheet(&format!("<xsl:template match='/'><out><xsl:text{attribute}>&lt;b/&gt;&amp;</xsl:text></out></xsl:template>"));
        assert_eq!(transform_str("<doc/>", &style), format!("<out>{expected}</out>"));
    }
}

#[test]
fn xslt_element_name_patterns_do_not_match_attributes() {
    for pattern in ["v", "*", "child::v", "child::*"] {
        let style = stylesheet(&format!("<xsl:template match='/'><xsl:apply-templates select='doc/@v'/></xsl:template><xsl:template match='{pattern}' priority='1'>WRONG</xsl:template><xsl:template match='@*'>attribute</xsl:template>"));
        assert_eq!(transform_str("<doc v='A'/>", &style), "attribute", "{pattern}");
    }
}

#[test]
fn xslt_builtin_rules_cover_each_node_kind_and_preserve_mode() {
    for mode in ["", " mode='m'"] {
        let style = stylesheet(&format!("<xsl:template match='/'><out><xsl:apply-templates select='doc/@* | doc/node()'{mode}/></out></xsl:template>"));
        assert_eq!(transform_str("<doc a='A'><v>2</v><v>10</v><!--C--><?pi P?></doc>", &style), "<out>A210</out>");
    }
    let style = stylesheet("<xsl:template match='/'><out><xsl:apply-templates select='doc' mode='m'/></out></xsl:template><xsl:template match='v' mode='m'>V</xsl:template><xsl:template match='v'>WRONG</xsl:template>");
    assert_eq!(transform_str("<doc><v>2</v><v>10</v></doc>", &style), "<out>VV</out>");
}

#[test]
fn xslt_avt_doubled_braces_and_braces_in_xpath_strings() {
    let style = stylesheet(r#"<xsl:template match="/"><out a="{{x}}" b="{{{doc/v}}}" c="{concat('}', '{')}"/></xsl:template>"#);
    assert_eq!(transform_str("<doc><v>2</v></doc>", &style), "<out a='{x}' b='{2}' c='}{'/>");
    for invalid in ["}", "{", "{1 +}", "{1 garbage}", "{1 { 2}"] {
        assert!(xrust::parser::avt::parse::<xrust::trees::smite::RNode>(invalid, None).is_err(), "{invalid}");
    }
}

#[test]
fn xslt_union_is_in_document_order_without_duplicate_nodes() {
    for (select, expected) in [
        ("doc/v[2] | doc/v[1]", "210"),
        ("doc/v | doc/v", "210"),
        ("doc/v[2] | doc/v | doc/v[1]", "210"),
    ] {
        let style = stylesheet(&format!("<xsl:template match='/'><out><xsl:for-each select='{select}'><xsl:value-of select='.'/></xsl:for-each></out></xsl:template>"));
        assert_eq!(transform_str("<doc><v>2</v><v>10</v></doc>", &style), format!("<out>{expected}</out>"));
    }
}
