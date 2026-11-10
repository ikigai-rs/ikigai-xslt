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
