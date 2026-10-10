//! The XSLT 1.0 subset xrust runs, MEASURED rather than remembered (ledger #193).
//!
//! gonk measured this on 2026-09-15 by rendering its pages through this crate; these tests
//! re-measure it on the xrust this crate builds against, so the README's subset table is
//! evidence. Three groups:
//!
//! - **works**: constructs that give the XSLT 1.0 answer;
//! - **refused**: constructs xrust rejects with an error, which is the safe failure, since a
//!   stylesheet author sees it;
//! - **silent**: constructs that give a WRONG answer with no error at all. Each `silent_*`
//!   test pins today's wrong output AND checks that stock xrust (its own `smite::RNode`,
//!   not this crate's `IdNode` wrapper) gives the same one, so the defect is xrust's and
//!   the reproduction in `docs/upstream-xrust.md` is honest.
//!
//! ★ **A failing `silent_*` test is GOOD NEWS, on purpose.** It means xrust changed its
//! answer: if it now gives the correct one, the failure says so. Either way, update the
//! "XSLT subset" section of `README.md` and `docs/upstream-xrust.md`, then the test. The
//! same goes for `the_subset_was_measured_on_xrust_2_2_0`, which fails on any other xrust:
//! the README names the version these answers were measured on.

use ikigai_xslt::transform_xml;
use xrust::item::{Item, Node, SequenceTrait};
use xrust::parser::xml::parse;
use xrust::parser::ParseError;
use xrust::transform::context::StaticContextBuilder;
use xrust::trees::smite::RNode;
use xrust::xdmerror::{Error, ErrorKind};
use xrust::xslt::from_document;

/// The xrust version every answer in this file was measured on, and the README states.
const MEASURED_ON: &str = "2.2.0";

/// A plain, namespace-free source.
const SRC: &str = r#"<doc title="T"><item id="a" k="1">one</item><item id="b" k="2">two</item><item id="c" k="1">three</item></doc>"#;

/// A namespaced source, shaped like the RDF/XML a page renders from.
const NS_SRC: &str = r#"<page xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:l="urn:l#"><l:Item rdf:about="urn:i:1">one</l:Item><l:Item rdf:about="urn:i:2">two</l:Item></page>"#;

/// A version 1.0 stylesheet with `method="xml"` around `templates`, declaring the prefixes
/// [`NS_SRC`] uses.
fn style(templates: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:l="urn:l#">
<xsl:output method="xml"/>
{templates}
</xsl:stylesheet>"#
    )
}

/// Through this crate, as every caller runs it.
fn ours(src: &str, templates: &str) -> Result<String, String> {
    transform_xml(src, &style(templates), false)
}

/// Through stock xrust, exactly as its own `xslt` module documentation drives it: the
/// reproduction an upstream report would carry.
fn stock(src: &str, templates: &str) -> Result<String, String> {
    fn doc(s: &str) -> Result<RNode, Error> {
        let d = RNode::new_document();
        parse(
            d.clone(),
            s,
            Some(|_: &_| Err(ParseError::MissingNameSpace)),
        )?;
        Ok(d)
    }
    let src = doc(src).map_err(|e| e.message)?;
    let styledoc = doc(&style(templates)).map_err(|e| e.message)?;
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "no fetcher")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "no parser")))
        .build();
    let mut ctxt = from_document(styledoc, None, doc, |_| Ok(String::new()))
        .map_err(|e| format!("compile: {}", e.message))?;
    ctxt.context(vec![Item::Node(src.clone())], 0);
    ctxt.result_document(RNode::new_document());
    ctxt.populate_key_values(&mut stctxt, src)
        .map_err(|e| format!("keys: {}", e.message))?;
    ctxt.evaluate(&mut stctxt)
        .map(|seq| seq.to_xml())
        .map_err(|e| format!("transform: {}", e.message))
}

/// Pin a silent case: today's answer is `wrong`, through this crate and through stock xrust
/// alike, with no error; XSLT 1.0 says `correct`.
fn pin_silent(src: &str, templates: &str, wrong: &str, correct: &str) {
    assert_ne!(wrong, correct, "a silent case pins a WRONG answer");
    for (path, answer) in [
        ("ikigai-xslt", ours(src, templates)),
        ("stock xrust", stock(src, templates)),
    ] {
        let answer = answer.unwrap_or_else(|e| {
            panic!("{path}: xrust now REFUSES this (`{e}`), which is better than a wrong answer: move it to the refused table in README.md and docs/upstream-xrust.md")
        });
        assert!(
            answer != correct,
            "{path}: xrust {MEASURED_ON}'s silent defect is FIXED (it now answers the XSLT 1.0 `{correct}`): update README.md's subset table and docs/upstream-xrust.md"
        );
        assert_eq!(
            answer, wrong,
            "{path}: xrust's answer changed but is still not `{correct}`: re-measure and update README.md and docs/upstream-xrust.md"
        );
    }
}

/// The xrust in this build, read from the lock file the build resolved.
fn xrust_in_this_build() -> String {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock"))
        .expect("cargo writes Cargo.lock before it builds a test");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line == r#"name = "xrust""# {
            let version = lines.next().expect("a version follows a name");
            return version
                .trim_start_matches("version = \"")
                .trim_end_matches('"')
                .to_string();
        }
    }
    panic!("no xrust in Cargo.lock")
}

#[test]
fn the_subset_was_measured_on_xrust_2_2_0() {
    assert_eq!(
        xrust_in_this_build(),
        MEASURED_ON,
        "this build uses another xrust than the README's subset table was measured on: \
         if every other test here passes, the table still holds, so update MEASURED_ON and \
         the version named in README.md and docs/upstream-xrust.md"
    );
}

// ---------------------------------------------------------------------------- works

#[test]
fn the_supported_subset_gives_the_xslt_answer() {
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "xsl:if",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:if test="@k='1'"><p><xsl:value-of select="."/></p></xsl:if></xsl:for-each></out></xsl:template>"#,
            "<out><p>one</p><p>three</p></out>",
        ),
        (
            "xsl:choose",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:choose><xsl:when test="@k='1'"><a/></xsl:when><xsl:otherwise><b/></xsl:otherwise></xsl:choose></xsl:for-each></out></xsl:template>"#,
            "<out><a/><b/><a/></out>",
        ),
        (
            "xsl:attribute, prefixed names included",
            NS_SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="page/l:Item"><a><xsl:attribute name="href"><xsl:value-of select="@rdf:about"/></xsl:attribute></a></xsl:for-each></out></xsl:template>"#,
            "<out><a href='urn:i:1'/><a href='urn:i:2'/></out>",
        ),
        (
            "call-template",
            SRC,
            r#"<xsl:template match="/"><out><xsl:call-template name="n"/></out></xsl:template><xsl:template name="n"><n/></xsl:template>"#,
            "<out><n/></out>",
        ),
        (
            "modes",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item" mode="m"/></out></xsl:template><xsl:template match="item" mode="m"><m/></xsl:template><xsl:template match="item"><x/></xsl:template>"#,
            "<out><m/><m/><m/></out>",
        ),
        (
            "xsl:sort inside apply-templates",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"><xsl:sort select="@id" order="descending"/></xsl:apply-templates></out></xsl:template><xsl:template match="item"><p><xsl:value-of select="."/></p></xsl:template>"#,
            "<out><p>three</p><p>two</p><p>one</p></out>",
        ),
        (
            "count()",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="count(doc/item)"/></out></xsl:template>"#,
            "<out>3</out>",
        ),
        (
            "a [@attr = '…'] filter on a select path",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item[@k='1']"><p><xsl:value-of select="."/></p></xsl:for-each></out></xsl:template>"#,
            "<out><p>one</p><p>three</p></out>",
        ),
        (
            "a [position() = n] filter (where [n] is ignored)",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="count(doc/item[position() = 1])"/></out></xsl:template>"#,
            "<out>1</out>",
        ),
        (
            "xsl:value-of over [position() = 1], the workaround for a node set",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="doc/item[position() = 1]"/></out></xsl:template>"#,
            "<out>one</out>",
        ),
        (
            "attribute value templates over UNPREFIXED names: functions, literals, parents, absolute paths",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><a href="x-{@id}" n="{concat(name(), @k)}" t="{../@title}{/doc/@title}"/></xsl:template>"#,
            "<out><a href='x-a' n='item1' t='TT'/><a href='x-b' n='item2' t='TT'/><a href='x-c' n='item1' t='TT'/></out>",
        ),
        (
            "an absolute /path from a nested template",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><p><xsl:value-of select="/doc/@title"/>:<xsl:value-of select="count(/doc//item)"/></p></xsl:template>"#,
            "<out><p>T:3</p><p>T:3</p><p>T:3</p></out>",
        ),
        (
            "a top-level xsl:variable",
            SRC,
            r#"<xsl:variable name="v" select="'V'"/><xsl:template match="/"><out><xsl:value-of select="$v"/></out></xsl:template>"#,
            "<out>V</out>",
        ),
        (
            "xsl:key over unprefixed names",
            SRC,
            r#"<xsl:key name="byk" match="item" use="@k"/><xsl:template match="/"><out><xsl:value-of select="count(key('byk','1'))"/></out></xsl:template>"#,
            "<out>2</out>",
        ),
        (
            "xsl:comment",
            SRC,
            r#"<xsl:template match="/"><out><xsl:comment>c</xsl:comment></out></xsl:template>"#,
            "<out><!--c--></out>",
        ),
    ];
    for (what, src, templates, expected) in cases {
        assert_eq!(ours(src, templates).as_deref(), Ok(*expected), "{what}");
    }
}

// -------------------------------------------------------------------------- refused

#[test]
fn unsupported_constructs_are_refused_by_name() {
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "xsl:variable inside a template",
            SRC,
            r#"<xsl:template match="/"><xsl:variable name="v" select="doc/@title"/><out><xsl:value-of select="$v"/></out></xsl:template>"#,
            r#"unsupported XSL element "variable""#,
        ),
        (
            "xsl:sort inside for-each",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:sort select="@id"/><p/></xsl:for-each></out></xsl:template>"#,
            r#"unsupported XSL element "sort""#,
        ),
        (
            "string-length()",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="string-length(doc/@title)"/></out></xsl:template>"#,
            r#"unknown callable "string-length""#,
        ),
        (
            "xsl:key over prefixed names",
            NS_SRC,
            r#"<xsl:key name="k" match="l:Item" use="@rdf:about"/><xsl:template match="/"><out><xsl:value-of select="count(key('k','urn:i:2'))"/></out></xsl:template>"#,
            "MissingNameSpace",
        ),
    ];
    for (what, src, templates, refusal) in cases {
        let err = ours(src, templates).expect_err(what);
        assert!(err.contains(refusal), "{what}: {err}");
    }

    // A stylesheet whose first node is a comment, not the xsl:stylesheet element.
    let err = transform_xml(
        SRC,
        &format!(
            "<!-- a comment -->{}",
            style(r#"<xsl:template match="/"><out/></xsl:template>"#)
        ),
        false,
    )
    .expect_err("a leading comment");
    assert!(err.contains("not an XSLT stylesheet"), "{err}");
}

// --------------------------------------------------------------------------- silent

/// An attribute value template that names a PREFIXED node is empty: `{@rdf:about}`,
/// `{l:child}`, and the prefixed name inside a function (`{concat('x', @rdf:about)}` is
/// `x`). Unprefixed names work, functions and absolute paths included (see the works
/// table); `xsl:attribute` with `xsl:value-of` is the workaround.
#[test]
fn silent_a_prefixed_name_in_an_attribute_value_template_is_empty() {
    pin_silent(
        NS_SRC,
        r#"<xsl:template match="/"><out><xsl:for-each select="page/l:Item"><a href="{@rdf:about}" t="{concat('x', @rdf:about)}"/></xsl:for-each></out></xsl:template>"#,
        "<out><a href='' t='x'/><a href='' t='x'/></out>",
        "<out><a href='urn:i:1' t='xurn:i:1'/><a href='urn:i:2' t='xurn:i:2'/></out>",
    );
}

/// A predicate in a MATCH pattern is ignored: `item[@k='2']` matches every `item`.
#[test]
fn silent_a_predicate_in_a_match_pattern_is_ignored() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item[@k='2']"><hit><xsl:value-of select="."/></hit></xsl:template>"#,
        "<out><hit>one</hit><hit>two</hit><hit>three</hit></out>",
        "<out>one<hit>two</hit>three</out>",
    );
}

/// With two templates for one name, the FIRST declared wins whatever its predicate: XSLT
/// gives `item[@k='1']` priority 0.5 over `item`'s 0, and xrust ignores both the predicate
/// and the priority.
#[test]
fn silent_template_priority_ignores_the_predicate_and_takes_the_first_declared() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><other/></xsl:template><xsl:template match="item[@k='1']"><one/></xsl:template>"#,
        "<out><other/><other/><other/></out>",
        "<out><one/><other/><one/></out>",
    );
}

/// A numeric predicate on a path is ignored: `item[1]` and `item[last()]` select every
/// `item`. `[position() = 1]` works (see the works table).
#[test]
fn silent_a_numeric_path_predicate_is_ignored() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:value-of select="count(doc/item[1])"/>,<xsl:value-of select="count(doc/item[last()])"/></out></xsl:template>"#,
        "<out>3,3</out>",
        "<out>1,1</out>",
    );
}

/// `position()` and `last()` are always 1 inside `for-each` and `apply-templates`, so a
/// test such as `position() = 2` is never true and its content is silently absent.
#[test]
fn silent_position_and_last_are_always_one() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><p><xsl:value-of select="position()"/>/<xsl:value-of select="last()"/></p></xsl:for-each><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><xsl:if test="position() = 2"><second/></xsl:if></xsl:template>"#,
        "<out><p>1/1</p><p>1/1</p><p>1/1</p></out>",
        "<out><p>1/3</p><p>2/3</p><p>3/3</p><second/></out>",
    );
}

/// `//name` is evaluated from the CONTEXT node, not the document root, so from inside a
/// nested template it finds only the context's own descendants (usually none). `/doc//name`
/// works (see the works table).
#[test]
fn silent_a_leading_double_slash_is_relative_to_the_context_node() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><p><xsl:value-of select="count(//item)"/></p></xsl:template>"#,
        "<out><p>0</p><p>0</p><p>0</p></out>",
        "<out><p>3</p><p>3</p><p>3</p></out>",
    );
}

/// `xsl:value-of` of several nodes concatenates them all with no separator. XSLT 1.0 takes
/// the FIRST node's string value (and a 2.0+ processor running a 1.0 stylesheet does too).
#[test]
fn silent_value_of_a_node_set_concatenates_every_node() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:value-of select="doc/item"/></out></xsl:template>"#,
        "<out>onetwothree</out>",
        "<out>one</out>",
    );
}

/// `xsl:output method="html"` still serializes XML: every empty element is self-closed,
/// and a browser reads `<script …/>` as an OPEN script element that swallows the rest of
/// the page (`<textarea/>` does the same to everything after it). Not compared against
/// stock xrust: the serializer is the same `to_xml` the stock runner calls.
#[test]
fn silent_html_output_is_serialized_as_xml() {
    let html = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:output method="html"/><xsl:template match="/"><html><head><script src="a.js"></script></head><body><textarea></textarea><br/></body></html></xsl:template></xsl:stylesheet>"#;
    let out = transform_xml(SRC, html, false).expect("it renders");
    assert_eq!(
        out, "<html><head><script src='a.js'/></head><body><textarea/><br/></body></html>",
        "the HTML serialization changed: if `<script>` now has a close tag, update the README's \
         serialization note and docs/upstream-xrust.md"
    );
}
