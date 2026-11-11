//! Local parameters are classed by what their call sites can pass (ledger #3398): a
//! declaration is itself, not every declaration spelled the same; a recursive template's
//! own calls take part in a bounded fixed point; XPath 1.0 core functions have fixed
//! result types. A parameter that can be a boolean or not was refused through 0.8.1 (#2520,
//! #2668); its value-of is now answered (ledger #3343), and its copy-of into content is still
//! refused (#3412).

const XSL: &str = "http://www.w3.org/1999/XSL/Transform";

fn transform(src: &str, stylesheet: &str) -> Result<String, String> {
    ikigai_xslt::transform_xml(src, stylesheet, true)
}

fn text_style(templates: &str) -> String {
    format!(
        "<xsl:stylesheet xmlns:xsl='{XSL}' version='1.0'><xsl:output method='text'/>{templates}</xsl:stylesheet>"
    )
}

fn assert_refused(style: &str) {
    let error = transform("<doc/>", style).unwrap_err();
    assert!(
        error.starts_with("invalid argument `stylesheet`: refused:") && error.contains("boolean"),
        "{error}"
    );
}

const LAST_SEGMENT: &str = "<xsl:template name='last-segment'><xsl:param name='iri'/>\
     <xsl:choose>\
     <xsl:when test=\"contains($iri, '/')\"><xsl:call-template name='last-segment'>\
     <xsl:with-param name='iri' select=\"substring-after($iri, '/')\"/></xsl:call-template></xsl:when>\
     <xsl:otherwise><xsl:value-of select='$iri'/></xsl:otherwise>\
     </xsl:choose></xsl:template>";

const SRC: &str = "<doc iri='http://example.org/a/b/c'/>";

#[test]
fn a_recursive_string_parameter_is_admitted() {
    let style = text_style(&format!(
        "<xsl:template match='/'><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='string(doc/@iri)'/></xsl:call-template></xsl:template>{LAST_SEGMENT}"
    ));
    assert_eq!(transform(SRC, &style).unwrap(), "c");
}

/// The FOAF refusal: `label` declares a parameter spelled exactly like `last-segment`'s, and
/// xrust's `==` on nodes is structural, so `$iri` forwarded from `label` read as a cycle.
#[test]
fn a_parameter_forwarded_from_a_same_named_parameter_is_admitted() {
    for caller in [
        "<xsl:template match='/'><xsl:call-template name='label'>\
         <xsl:with-param name='iri' select='string(doc/@iri)'/></xsl:call-template></xsl:template>\
         <xsl:template name='label'><xsl:param name='iri'/><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='$iri'/></xsl:call-template></xsl:template>",
        // Both callers of `last-segment` forward a same-named parameter.
        "<xsl:template match='/'><xsl:call-template name='label'>\
         <xsl:with-param name='iri' select='string(doc/@iri)'/></xsl:call-template>\
         <xsl:call-template name='title'><xsl:with-param name='iri' select=\"concat(doc/@iri, '')\"/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='label'><xsl:param name='iri'/><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='$iri'/></xsl:call-template></xsl:template>\
         <xsl:template name='title'><xsl:param name='iri'/><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='$iri'/></xsl:call-template></xsl:template>",
    ] {
        let style = text_style(&format!("{caller}{LAST_SEGMENT}"));
        assert!(transform(SRC, &style).unwrap().starts_with('c'), "{caller}");
    }
}

/// Identity matters in scope resolution too: the second `value-of` is not the first, so it
/// sees the boolean variable declared between them rather than the global string.
#[test]
fn identical_instructions_resolve_their_own_bindings() {
    let style = text_style(
        "<xsl:variable name='v' select=\"'s'\"/><xsl:template match='/'>\
         <xsl:value-of select='$v'/><xsl:variable name='v' select='true()'/><xsl:value-of select='$v'/>\
         </xsl:template>",
    );
    assert_eq!(transform("<doc/>", &style).unwrap(), "strue");
}

#[test]
fn a_recursive_call_passing_its_own_parameter_takes_the_other_sites_kind() {
    // `sep` is forwarded unchanged on every recursive call.
    let style = text_style(
        "<xsl:template match='/'><xsl:call-template name='join'>\
         <xsl:with-param name='rest' select=\"'a b c'\"/><xsl:with-param name='sep' select=\"concat(',', '')\"/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='join'><xsl:param name='rest'/><xsl:param name='sep'/>\
         <xsl:value-of select=\"substring-before(concat($rest, ' '), ' ')\"/>\
         <xsl:if test=\"contains($rest, ' ')\"><xsl:value-of select='$sep'/>\
         <xsl:call-template name='join'><xsl:with-param name='rest' select=\"substring-after($rest, ' ')\"/>\
         <xsl:with-param name='sep' select='$sep'/></xsl:call-template></xsl:if></xsl:template>",
    );
    assert_eq!(transform("<doc/>", &style).unwrap(), "a,b,c");
    // A boolean forwarded the same way is a boolean on every call, and is adapted. The
    // default is one of the values a parameter can take, so it is a boolean too.
    let style = text_style(
        "<xsl:template match='/'><xsl:call-template name='flag'>\
         <xsl:with-param name='n' select='2'/><xsl:with-param name='b' select='true()'/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='flag'><xsl:param name='n'/><xsl:param name='b' select='false()'/>\
         <xsl:value-of select='$b'/><xsl:if test='$n &gt; 1'><xsl:call-template name='flag'>\
         <xsl:with-param name='n' select='$n - 1'/><xsl:with-param name='b' select='$b'/>\
         </xsl:call-template></xsl:if></xsl:template>",
    );
    assert_eq!(transform("<doc/>", &style).unwrap(), "truetrue");
}

#[test]
fn a_genuinely_mixed_parameter_is_answered_and_its_copy_refused() {
    // One outside caller passes true(): `iri` can be a boolean or a string.
    let answer = |style: &str| transform(SRC, style).unwrap();
    assert_eq!(
        answer(&text_style(&format!(
            "<xsl:template match='/'><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='string(doc/@iri)'/></xsl:call-template>\
         <xsl:call-template name='last-segment'><xsl:with-param name='iri' select='true()'/>\
         </xsl:call-template></xsl:template>{LAST_SEGMENT}"
        ))),
        "ctrue"
    );
    // The forwarding template's own caller passes true().
    assert_eq!(answer(&text_style(&format!(
        "<xsl:template match='/'><xsl:call-template name='label'>\
         <xsl:with-param name='iri' select='true()'/></xsl:call-template>\
         <xsl:call-template name='last-segment'><xsl:with-param name='iri' select='string(doc/@iri)'/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='label'><xsl:param name='iri'/><xsl:call-template name='last-segment'>\
         <xsl:with-param name='iri' select='$iri'/></xsl:call-template></xsl:template>{LAST_SEGMENT}"
    ))), "truec");
    // Copied into a variable, where an atomic boolean would leak (#3412), the analysis still
    // decides. These two recurse without end once they run, so the refusal is all they give.
    // Only the recursion turns the string into a boolean: no guess holds.
    assert_refused(&text_style(
        "<xsl:template match='/'><xsl:call-template name='r'><xsl:with-param name='p' select=\"'s'\"/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='r'><xsl:param name='p'/><xsl:variable name='c'><xsl:copy-of select='$p'/></xsl:variable>\
         <xsl:if test=\"$p = 's'\"><xsl:call-template name='r'>\
         <xsl:with-param name='p' select=\"$p = 's'\"/></xsl:call-template></xsl:if></xsl:template>",
    ));
    // The recursion may pass either kind back.
    assert_refused(&text_style(
        "<xsl:template match='/'><xsl:call-template name='r'><xsl:with-param name='p' select=\"'s'\"/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='r'><xsl:param name='p'/><xsl:variable name='c'><xsl:copy-of select='$p'/></xsl:variable>\
         <xsl:if test=\"$p = 's'\"><xsl:call-template name='r'>\
         <xsl:with-param name='p' select=\"if ($p = 's') then $p else true()\"/></xsl:call-template></xsl:if></xsl:template>",
    ));
}

#[test]
fn mutually_recursive_parameters_are_admitted_and_bounded() {
    let style = text_style(
        "<xsl:template match='/'><xsl:call-template name='a'><xsl:with-param name='p' select=\"'x/y/z'\"/>\
         </xsl:call-template></xsl:template>\
         <xsl:template name='a'><xsl:param name='p'/><xsl:choose><xsl:when test=\"contains($p, '/')\">\
         <xsl:call-template name='b'><xsl:with-param name='q' select=\"substring-after($p, '/')\"/>\
         </xsl:call-template></xsl:when><xsl:otherwise><xsl:value-of select='$p'/></xsl:otherwise></xsl:choose></xsl:template>\
         <xsl:template name='b'><xsl:param name='q'/><xsl:call-template name='a'>\
         <xsl:with-param name='p' select='$q'/></xsl:call-template></xsl:template>",
    );
    assert_eq!(transform("<doc/>", &style).unwrap(), "z");
}

/// A chain of recursive templates, each forwarding its parameter to itself and to the next,
/// from a caller passing true(). Each level's string guess fails before its boolean guess
/// holds, and each guess analyzes the level below again, so the passes double per level.
/// A total budget of declaration visits refuses it instead of running 2^n passes. The sink
/// is a copy-of into a variable, which the analysis still decides (#3412); a value-of no
/// longer needs it (ledger #3343).
#[test]
fn deep_recursive_forwarding_is_bounded() {
    let depth = ikigai_xslt::limits::MAX_EXPR_DEPTH - 4;
    let mut templates = String::from(
        "<xsl:template match='/'><xsl:call-template name='t0'><xsl:with-param name='p' select='true()'/>\
         </xsl:call-template></xsl:template>",
    );
    for i in 0..depth {
        templates.push_str(&format!(
            "<xsl:template name='t{i}'><xsl:param name='p'/><xsl:param name='n' select='0'/>\
             <xsl:if test='$n'><xsl:call-template name='t{i}'><xsl:with-param name='p' select='$p'/>\
             </xsl:call-template></xsl:if>\
             <xsl:call-template name='t{}'><xsl:with-param name='p' select='$p'/></xsl:call-template></xsl:template>",
            i + 1
        ));
    }
    templates.push_str(&format!(
        "<xsl:template name='t{depth}'><xsl:param name='p'/><xsl:variable name='c'><xsl:copy-of select='$p'/></xsl:variable></xsl:template>"
    ));
    let started = std::time::Instant::now();
    assert_refused(&text_style(&templates));
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn core_functions_have_their_fixed_result_types() {
    let src = "<doc xml:lang='en' n='2.5'><a>x</a></doc>";
    // Each is passed through a parameter, so its kind decides the value-of. (sum, lang and
    // namespace-uri are in the unit table in src/boolean_values.rs: xrust 2.3.0 answers
    // sum(@n) NaN and implements neither of the other two.)
    for (expression, expected) in [
        ("string(doc/a)", "x"),
        ("concat('a', 'b')", "ab"),
        ("substring('abc', 2)", "bc"),
        ("substring-before('a/b', '/')", "a"),
        ("substring-after('a/b', '/')", "b"),
        ("normalize-space(' a  b ')", "a b"),
        ("translate('abc', 'b', 'x')", "axc"),
        ("local-name(doc)", "doc"),
        ("name(doc)", "doc"),
        ("count(doc/a)", "1"),
        ("number(doc/@n)", "2.5"),
        ("string-length('abc')", "3"),
        ("floor(2.5)", "2"),
        ("ceiling(2.5)", "3"),
        ("round(2.4)", "2"),
        ("position()", "1"),
        ("last()", "1"),
        ("boolean(doc/a)", "true"),
        ("not(doc/a)", "false"),
        ("true()", "true"),
        ("false()", "false"),
        ("contains('abc', 'b')", "true"),
        ("starts-with('abc', 'b')", "false"),
    ] {
        // The default is a value too: a boolean argument needs a boolean default.
        let default = if matches!(expected, "true" | "false") {
            " select='false()'"
        } else {
            ""
        };
        let style = text_style(&format!(
            "<xsl:template match='/'><xsl:call-template name='show'>\
             <xsl:with-param name='p' select=\"{expression}\"/></xsl:call-template></xsl:template>\
             <xsl:template name='show'><xsl:param name='p'{default}/><xsl:value-of select='$p'/></xsl:template>"
        ));
        assert_eq!(transform(src, &style).unwrap(), expected, "{expression}");
    }
    // An extension function's result type is unknown, here and in a string conversion.
    // Through 0.8.1 that was refused; now the value-of runs and xrust reports the
    // function it does not know (ledger #3343).
    for select in ["$p", "string($p)", "ex:f()", "string(ex:f())"] {
        let style = format!(
            "<xsl:stylesheet xmlns:xsl='{XSL}' xmlns:ex='urn:ex' version='1.0'>\
             <xsl:template match='/'><xsl:call-template name='show'>\
             <xsl:with-param name='p' select='ex:f()'/></xsl:call-template></xsl:template>\
             <xsl:template name='show'><xsl:param name='p'/><xsl:value-of select='{select}'/></xsl:template>\
             </xsl:stylesheet>"
        );
        let error = transform("<doc/>", &style).unwrap_err();
        assert!(
            error.starts_with("invalid argument `stylesheet`: refused:")
                || error.contains("unknown callable \"{urn:ex}f\""),
            "{select}: {error}"
        );
    }
}

#[test]
fn the_foaf_parameter_shape_renders_every_section() {
    let page = ikigai_xslt::transform_xml(
        include_str!("fixtures/foaf-last-segment.rdf"),
        include_str!("fixtures/foaf-last-segment.xsl"),
        false,
    )
    .unwrap();
    for eyebrow in [
        "Person",
        "Links",
        "Knows",
        "Interests",
        "Accounts",
        "Projects",
    ] {
        assert!(
            page.contains(&format!("<p class='eyebrow'>{eyebrow}</p>")),
            "{eyebrow}: {page}"
        );
    }
    for label in [
        "<p>ada</p>",
        ">home</a>",
        ">blog</a>",
        ">grace</a>",
        ">engines</a>",
        ">ada</a>",
        ">analytical</a>",
    ] {
        assert!(page.contains(label), "{label}: {page}");
    }
}
