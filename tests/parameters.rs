use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
use std::sync::Arc;

const STYLE: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:param name="feed" select="'default feed'"/>
  <xsl:param name="terms">default terms</xsl:param>
  <xsl:template match="/"><xsl:value-of select="$feed"/>|<xsl:value-of select="$terms"/></xsl:template>
</xsl:stylesheet>"#;

fn request(style: &str, args: &[(&str, &str)]) -> Request {
    let mut req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(style.as_bytes().to_vec()));
    for (name, value) in args {
        req = req.with_arg(*name, ArgRef::Inline(value.as_bytes().to_vec()));
    }
    req
}

fn issue(kernel: &Kernel, style: &str, args: &[(&str, &str)]) -> Result<String, Error> {
    futures::executor::block_on(kernel.issue(request(style, args), &Capability::root()))
        .map(|repr| String::from_utf8(repr.bytes).unwrap())
}

#[test]
fn request_arguments_reach_top_level_parameters() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    assert_eq!(
        issue(
            &kernel,
            STYLE,
            &[
                ("feed", "https://example.org/feed.xml"),
                ("terms", "rust & XML")
            ]
        )
        .unwrap(),
        "https://example.org/feed.xml|rust & XML"
    );
}

#[test]
fn defaults_and_empty_strings_are_independent_between_requests() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for (args, expected) in [
        (vec![("feed", "one"), ("terms", "two")], "one|two"),
        (vec![], "default feed|default terms"),
        (vec![("feed", "")], "|default terms"),
        (vec![("terms", "three")], "default feed|three"),
    ] {
        assert_eq!(issue(&kernel, STYLE, &args).unwrap(), expected);
    }
}

#[test]
fn only_declared_top_level_parameters_are_accepted() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for name in ["typo", "local", "fixed"] {
        let style = STYLE.replace("<xsl:template match=\"/\">", "<xsl:variable name=\"fixed\" select=\"'constant'\"/><xsl:template name=\"unused\"><xsl:param name=\"local\"/></xsl:template><xsl:template match=\"/\">");
        match issue(&kernel, &style, &[(name, "x")]) {
            Err(Error::InvalidArgument { name: got, .. }) => assert_eq!(got, name),
            other => panic!("expected InvalidArgument for {name}, got {other:?}"),
        }
    }
}

#[test]
fn supplied_values_are_literal_strings_and_part_of_request_identity() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for value in [
        "'\"<& $feed /doc",
        "1 + 2",
        "https://example.org/?a=1&b=2",
        "second",
    ] {
        let args = [("feed", value), ("terms", "")];
        assert_eq!(issue(&kernel, STYLE, &args).unwrap(), format!("{value}|"));
        assert!(kernel.is_cached(&request(STYLE, &args), &Capability::root()));
    }
}

#[test]
fn selected_parameters_reach_dependent_defaults_and_variables() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
      <xsl:output method="text"/>
      <xsl:param name="feed" select="/doc/@url"/>
      <xsl:param name="terms" select="concat($feed, ' search')"/>
      <xsl:param name="empty"/>
      <xsl:param name="number" select="2"/>
      <xsl:param name="false" select="false()"/>
      <xsl:variable name="title" select="concat($terms, '!')"/>
      <xsl:template match="/"><xsl:value-of select="$title"/>|<xsl:value-of select="$empty"/>|<xsl:value-of select="$number + 1"/>|<xsl:if test="$false">bad</xsl:if></xsl:template>
    </xsl:stylesheet>"#;
    assert_eq!(
        issue(&kernel, style, &[("feed", "my feed")]).unwrap(),
        "my feed search!||3|"
    );
    assert_eq!(issue(&kernel, style, &[]).unwrap(), " search!||3|");
}

#[test]
fn bindings_do_not_shadow_names_hidden_by_character_references() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = STYLE.replace(
        "<xsl:param name=\"feed\" select=\"'default feed'\"/>",
        "<xsl:variable name=\"ikigai&#95;xslt_parameter_0s\" select=\"'default feed'\"/><xsl:param name=\"feed\" select=\"$ikigai&#95;xslt_parameter_0s\"/>",
    );
    assert_eq!(
        issue(&kernel, &style, &[]).unwrap(),
        "default feed|default terms"
    );
}

#[test]
fn referenced_stylesheet_and_content_input_accept_parameters() {
    use ikigai_core::{Description, Exact, FnEndpoint, Invocation, ReprType, Representation};
    let stylesheet = FnEndpoint::new("parameter-style", |_inv: &Invocation<'_>| {
        Ok(
            Representation::new(ReprType::new("application/xml"), STYLE.as_bytes().to_vec())
                .cacheable()
                .depends_on("urn:test:style"),
        )
    })
    .with_description(
        Description::new("parameter-style")
            .verb(Verb::Source)
            .output("application/xml"),
    );
    let kernel = Kernel::new(Arc::new(
        ikigai_xslt::space().bind(Exact::new("urn:test:style"), stylesheet),
    ));
    let mut req = request("urn:test:style", &[("feed", "from content")]);
    req.args.remove("src");
    req = req.with_arg("content", ArgRef::Inline(b"<doc/>".to_vec()));
    let repr = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert!(repr
        .threads()
        .iter()
        .any(|t| t.to_string() == "urn:test:style"));
    assert_eq!(
        String::from_utf8(repr.bytes).unwrap(),
        "from content|default terms"
    );
}

#[test]
fn contract_admits_named_parameters() {
    use ikigai_core::{Arguments, ProposedArgs};
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let description = kernel.describe_pattern("urn:xslt:transform").unwrap();
    let action = description
        .action_specs()
        .into_iter()
        .find(|a| a.verb == Verb::Source)
        .unwrap();
    assert!(matches!(action.arguments, Arguments::Open(_)));
    assert!(action
        .check(&ProposedArgs::parse(
            "src=urn:doc&stylesheet=urn:style&feed=https://example.org/feed&terms=rust"
        ))
        .is_empty());
}

#[test]
fn parameter_values_must_be_inline_utf8() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = request(STYLE, &[]).with_arg("feed", ArgRef::Inline(vec![0xff]));
    match futures::executor::block_on(kernel.issue(req, &Capability::root())) {
        Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "feed"),
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

/// Since ikigai-core 0.2.5 (ledger #2511) the kernel settles a by-reference argument by the
/// target's contract before the endpoint runs: an argument of an OPEN set, as every
/// stylesheet parameter is, receives the referent's bytes. So a parameter passed as
/// `feed=<urn:…>` reaches the stylesheet as the referent's text.
#[test]
fn a_parameter_passed_by_reference_receives_the_referents_text() {
    use ikigai_core::{
        EndpointSpace, Exact, Fallback, FnEndpoint, Invocation, ReprType, Representation, Space,
    };
    let value = FnEndpoint::new("value", |_: &Invocation<'_>| {
        Ok(Representation::new(
            ReprType::new("text/plain"),
            b"https://example.org/ref.xml".to_vec(),
        ))
    });
    let spaces: Vec<Arc<dyn Space>> = vec![
        Arc::new(ikigai_xslt::space()),
        Arc::new(EndpointSpace::new().bind(Exact::new("urn:test:value"), value)),
    ];
    let kernel = Kernel::new(Arc::new(Fallback::new(spaces)));
    let req = request(STYLE, &[]).with_arg(
        "feed",
        ArgRef::Reference(Iri::parse("urn:test:value").unwrap()),
    );
    let answer = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert_eq!(
        String::from_utf8(answer.bytes).unwrap(),
        "https://example.org/ref.xml|default terms"
    );
}

#[test]
fn malformed_parameter_declarations_are_refused() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for declaration in [
        "<xsl:param/>",
        "<xsl:param name='a:b'/>",
        "<xsl:param name='feed'/><xsl:param name='feed'/>",
    ] {
        let style = format!("<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>{declaration}<xsl:template match='/'/></xsl:stylesheet>");
        match issue(&kernel, &style, &[]) {
            Err(Error::InvalidArgument { name, .. }) => assert_eq!(name, "stylesheet"),
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }
}

#[test]
fn select_defaults_keep_source_node_identity() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
      <xsl:output method="text"/>
      <xsl:param name="node" select="/doc"/>
      <xsl:param name="document" select="/"/>
      <xsl:template match="/"><xsl:value-of select="generate-id($node)"/>|<xsl:value-of select="generate-id($document)"/></xsl:template>
    </xsl:stylesheet>"#;
    assert_eq!(issue(&kernel, style, &[]).unwrap(), "d1c1|d1");
}

#[test]
fn parameters_render_feed_iris_and_search_titles_as_xml() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
      <xsl:output method="xml"/>
      <xsl:param name="feed"/><xsl:param name="terms"/>
      <xsl:template match="/"><rss><channel xmlUrl="{$feed}"><title><xsl:value-of select="$terms"/></title></channel></rss></xsl:template>
    </xsl:stylesheet>"#;
    let out = issue(
        &kernel,
        style,
        &[
            ("feed", "https://example.org/feed?a=1&b=2"),
            ("terms", "Rust <XML>"),
        ],
    )
    .unwrap();
    assert!(
        out.contains("https://example.org/feed?a=1&amp;b=2"),
        "{out}"
    );
    assert!(out.contains("<title>Rust &lt;XML&gt;</title>"), "{out}");
}

#[test]
fn supplying_a_parameter_does_not_evaluate_its_default() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = STYLE.replace(
        "select=\"'default feed'\"",
        "select=\"document('urn:unused')\"",
    );
    assert_eq!(
        issue(&kernel, &style, &[("feed", "supplied")]).unwrap(),
        "supplied|default terms"
    );
    assert!(
        matches!(issue(&kernel, &style, &[]), Err(Error::Endpoint(message)) if message.contains("document()"))
    );
}

#[test]
fn defaults_keep_namespaces_declared_on_the_parameter() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let style = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
      <xsl:output method="text"/>
      <xsl:param xmlns:d="urn:doc" name="title" select="/d:doc/d:title"/>
      <xsl:template match="/"><xsl:value-of select="$title"/></xsl:template>
    </xsl:stylesheet>"#;
    let req = request(style, &[]).with_arg(
        "src",
        ArgRef::Inline(b"<doc xmlns='urn:doc'><title>Default title</title></doc>".to_vec()),
    );
    let repr = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert_eq!(String::from_utf8(repr.bytes).unwrap(), "Default title");
}
