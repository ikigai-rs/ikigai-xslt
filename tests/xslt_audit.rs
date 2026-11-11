//! Synthetic XSLT 1.0 / XPath 1.0 audit cases. No network access or external suite.
use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
use ikigai_xslt::{transform_xml, CompiledStylesheet};
use std::sync::Arc;

const SRC: &str = "<doc a='A'><v>2</v><v>10</v><!--C--><?pi P?></doc>";

fn style(declarations: &str, body: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:p="urn:p">{declarations}<xsl:template match="/">{body}</xsl:template></xsl:stylesheet>"#
    )
}

fn issue(src: &str, stylesheet: &str) -> Result<String, Error> {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(src.as_bytes().to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(stylesheet.as_bytes().to_vec()));
    block_on(kernel.issue(request, &Capability::root()))
        .map(|reply| String::from_utf8(reply.bytes).unwrap())
}

#[test]
fn value_of_uses_the_first_node_in_document_order() {
    for (select, expected) in [
        ("doc/v", "2"),
        ("doc/missing", ""),
        ("doc/v[2] | doc/v[1]", "2"),
        ("doc/v[2]/preceding-sibling::v", "2"),
        ("true()", "true"),
        ("false()", "false"),
        ("10", "10"),
    ] {
        let stylesheet = style("", &format!("<out><xsl:value-of select='{select}'/></out>"));
        let expected = format!("<out>{expected}</out>");
        assert_eq!(
            transform_xml(SRC, &stylesheet, false).unwrap(),
            expected,
            "{select}"
        );
        assert_eq!(issue(SRC, &stylesheet).unwrap(), expected, "{select}");
    }
}

#[test]
fn numeric_strings_signs_and_large_integer_literals_are_adapted() {
    for (select, expected) in [
        ("'10' &lt; '2'", "false"),
        ("'bad' &gt; '2'", "false"),
        ("-2", "-2"),
        ("- -2", "2"),
        ("18446744073709551616 = 18446744073709551617", "true"),
    ] {
        let stylesheet = style(
            "",
            &format!("<out><xsl:value-of select=\"{select}\"/></out>"),
        );
        assert_eq!(
            transform_xml(SRC, &stylesheet, false).unwrap(),
            format!("<out>{expected}</out>"),
            "{select}"
        );
    }
}

#[test]
fn text_escaping_and_disable_output_escaping_work_in_each_method() {
    for method in ["xml", "html", "text"] {
        for (hint, markup) in [
            ("", "&lt;b/&gt;&amp;"),
            (" disable-output-escaping='no'", "&lt;b/&gt;&amp;"),
            (" disable-output-escaping='yes'", "<b/>&"),
        ] {
            let sheet = style(
                &format!("<xsl:output method='{method}'/>"),
                &format!("<out><xsl:text{hint}>&lt;b/&gt;&amp;</xsl:text></out>"),
            );
            let expected = if method == "text" {
                "<b/>&".into()
            } else {
                format!("<out>{markup}</out>")
            };
            assert_eq!(
                transform_xml(SRC, &sheet, method == "text").unwrap(),
                expected
            );
            assert_eq!(issue(SRC, &sheet).unwrap(), expected);
        }
    }
    let sheet = style(
        "",
        "<out><xsl:value-of select=\"'&lt;b/&gt;'\" disable-output-escaping='yes'/></out>",
    );
    assert_eq!(issue(SRC, &sheet).unwrap(), "<out><b/></out>");
}

#[test]
fn avts_honor_escaped_braces_and_quoted_braces() {
    let sheet = style(
        "",
        r#"<out a="{{x}}" b="{{{doc/v[1]}}}" c="{concat('}', '{')}"/>"#,
    );
    assert_eq!(issue(SRC, &sheet).unwrap(), "<out a='{x}' b='{2}' c='}{'/>");
}

#[test]
fn builtins_copy_text_and_attributes_ignore_comments_and_preserve_modes() {
    for mode in ["", " mode='m'"] {
        let sheet = style(
            "",
            &format!("<out><xsl:apply-templates select='doc/@* | doc/node()'{mode}/></out>"),
        );
        assert_eq!(issue(SRC, &sheet).unwrap(), "<out>A210</out>");
    }
    let sheet = style("<xsl:template match='v' mode='m'>V</xsl:template><xsl:template match='v'>WRONG</xsl:template>", "<out><xsl:apply-templates select='doc' mode='m'/></out>");
    assert_eq!(issue(SRC, &sheet).unwrap(), "<out>VV</out>");
    // No explicit root or element template: both built-in recursion rules run.
    let sheet = "<xsl:stylesheet version='1.0' xmlns:xsl='http://www.w3.org/1999/XSL/Transform'/>";
    assert_eq!(issue(SRC, sheet).unwrap(), "210");
}

#[test]
fn element_name_patterns_do_not_match_attributes() {
    let sheet = style("<xsl:template match='v'>WRONG</xsl:template><xsl:template match='@*'>attribute</xsl:template>", "<xsl:apply-templates select='doc/@v'/>");
    assert_eq!(issue("<doc v='A'/>", &sheet).unwrap(), "attribute");
}

#[test]
fn default_and_explicit_priorities_select_the_most_specific_rule() {
    let sheet = style("<xsl:template match='*'>wild;</xsl:template><xsl:template match='p:*'>ns;</xsl:template><xsl:template match='p:v'>name;</xsl:template><xsl:template match='doc/p:v'>path;</xsl:template>", "<xsl:apply-templates select='doc/* | doc/p:n/*'/>");
    assert_eq!(
        issue(
            "<doc xmlns:p='urn:p'><plain/><p:v/><p:n><p:v/></p:n></doc>",
            &sheet
        )
        .unwrap(),
        "wild;path;ns;name;"
    );
    let sheet = style("<xsl:template match='v' priority='2'>high;</xsl:template><xsl:template match='v' priority='1'>low;</xsl:template>", "<xsl:apply-templates select='doc/v'/>");
    assert_eq!(issue(SRC, &sheet).unwrap(), "high;high;");
}

#[test]
fn unsupported_output_and_patterns_have_named_typed_refusals() {
    for (declaration, construct) in [
        (
            "<xsl:namespace-alias stylesheet-prefix='p' result-prefix='xsl'/>",
            "xsl:namespace-alias",
        ),
        (
            "<xsl:output method='xml' omit-xml-declaration='no'/>",
            "omit-xml-declaration",
        ),
        ("<xsl:output encoding='UTF-16'/>", "encoding"),
        (
            "<xsl:template match='//v'>V</xsl:template>",
            "leading // in a match pattern",
        ),
        (
            "<xsl:template match='v[1]'>V</xsl:template>",
            "predicate in a match pattern",
        ),
    ] {
        let err = issue(SRC, &style(declaration, "<out/>")).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains(construct)),
            "{construct}: {err}"
        );
    }
}

#[test]
fn xml_utf8_omission_excluded_prefixes_and_repeated_runs_are_stable() {
    let sheet = style(
        "<xsl:output method='xml' encoding='UTF-8' omit-xml-declaration='yes'/>",
        "<out xsl:exclude-result-prefixes='p'>é<xsl:value-of select='doc/v[1]'/></out>",
    );
    let compiled = CompiledStylesheet::compile(&sheet).unwrap();
    for _ in 0..5 {
        assert_eq!(compiled.transform(SRC, false).unwrap(), "<out>é2</out>");
    }
}

#[test]
fn raw_html_text_and_recursion_limits_refuse_the_whole_result() {
    for element in ["script", "style", "SCRIPT", "STYLE"] {
        let sheet = style(
            "<xsl:output method='html'/>",
            &format!("<out>partial<{element}>a &lt; b</{element}></out>"),
        );
        let err = issue(SRC, &sheet).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("inline HTML")),
            "{err}"
        );
    }
    let sheet = style(
        "<xsl:template name='loop'>partial<xsl:call-template name='loop'/></xsl:template>",
        "<out>partial<xsl:call-template name='loop'/></out>",
    );
    let err = issue(SRC, &sheet).unwrap_err();
    assert!(
        matches!(&err, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("depth") && detail.contains("recursion")),
        "{err}"
    );
}

#[test]
fn foaf_fixture_is_byte_identical_to_the_pre_audit_baseline() {
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
fn string_consumers_convert_the_first_node_and_large_integers_are_doubles() {
    for (expression, expected) in [
        ("string(doc/v)", "2"),
        ("string-length(doc/v)", "1"),
        ("concat(doc/v, '!')", "2!"),
        ("contains(doc/v, '10')", "false"),
        ("9007199254740992 = 9007199254740993", "true"),
    ] {
        let sheet = style("", &format!("<xsl:value-of select=\"{expression}\"/>"));
        assert_eq!(issue(SRC, &sheet).unwrap(), expected, "{expression}");
    }
}

#[test]
fn integer_literals_above_double_precision_round_as_xpath_1_0_numbers() {
    let sheet = style(
        "",
        "<xsl:value-of select='9007199254740992 = 9007199254740993'/>",
    );
    assert_eq!(issue(SRC, &sheet).unwrap(), "true");
}

#[test]
fn avts_convert_only_the_first_selected_node() {
    assert_eq!(
        issue(SRC, &style("", "<out n='{doc/v}'/>")).unwrap(),
        "<out n='2'/>"
    );
}

#[test]
fn output_declarations_merge_without_imports_too() {
    let sheet = style(
        "<xsl:output method='xml'/><xsl:output method='text'/>",
        "<out>text</out>",
    );
    assert_eq!(issue(SRC, &sheet).unwrap(), "text");
}

#[test]
fn a_named_root_rule_cannot_bypass_context_sensitive_refusals() {
    let sheet = "<xsl:stylesheet version='1.0' xmlns:xsl='http://www.w3.org/1999/XSL/Transform'><xsl:template match='/' name='both'><xsl:choose><xsl:when test='not(parent::node())'><xsl:for-each select='doc/v'><xsl:call-template name='both'/></xsl:for-each></xsl:when><xsl:otherwise><xsl:value-of select='position()'/></xsl:otherwise></xsl:choose></xsl:template></xsl:stylesheet>";
    let error = issue(SRC, sheet).expect_err("a named call can supply a non-root context");
    assert!(
        matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("position()")),
        "{error}"
    );
}

#[test]
fn output_method_inspection_agrees_with_compilation_after_merging() {
    let sheet = style(
        "<xsl:output method='xml'/><xsl:output method='text'/>",
        "text",
    );
    assert_eq!(
        ikigai_xslt::stylesheet_output_method(&sheet)
            .unwrap()
            .as_deref(),
        Some("text")
    );
    assert_eq!(
        CompiledStylesheet::compile(&sheet).unwrap().output_method(),
        Some("text")
    );
}
