//! The XSLT 1.0 subset xrust runs, MEASURED rather than remembered (ledger #193).
//!
//! gonk measured this on 2026-09-15 by rendering its pages through this crate; these tests
//! re-measure it on the xrust this crate builds against, so the README's subset table is
//! evidence. Four groups:
//!
//! - **works**: constructs that give the XSLT 1.0 answer;
//! - **refused**: constructs xrust rejects with an error, which is the safe failure, since a
//!   stylesheet author sees it;
//! - **refused here**: constructs xrust answers WRONGLY with no error at all, which this
//!   crate therefore refuses before xrust compiles them (`src/subset.rs`, since 0.3.0;
//!   prefixed attribute values are checked before parsing in `src/limits.rs`). Each
//!   `refused_*` test checks the refusal names the construct, AND that stock xrust (its own
//!   `smite::RNode`, not this crate's `IdNode` wrapper) still gives the pinned wrong answer,
//!   so the defect is xrust's, the refusal is still earning its keep, and the reproduction in
//!   `docs/upstream-xrust.md` is honest;
//! - **silent**: wrong answers still returned without an error, pinned
//!   the same way through both paths.
//!
//! ★ **A failing `refused_*` or `silent_*` test is GOOD NEWS, on purpose.** It means xrust
//! changed its answer: if it now gives the correct one, the failure says so, and the refusal
//! in `src/subset.rs` can go. Either way, update the "XSLT subset" section of `README.md` and
//! `docs/upstream-xrust.md`, then the test.
//!
//! The xrust release these answers were measured on is RECORDED, not gated
//! (`ikigai_xslt::subset::XRUST_MEASURED_ON`, Brian's decision on ledger #193): a newer xrust on
//! crates.io is a warning in the ecosystem's daily scan, and a build on another xrust prints a
//! note here. What fails is an answer that changed.

use ikigai_xslt::transform_xml;
use xrust::item::{Item, Node, SequenceTrait};
use xrust::parser::xml::parse;
use xrust::parser::ParseError;
use xrust::transform::context::StaticContextBuilder;
use xrust::trees::smite::RNode;
use xrust::xdmerror::{Error, ErrorKind};
use xrust::xslt::from_document;

/// The xrust version every answer in this file was measured on, and the README states.
/// ⚠ Keep this line's exact shape: ikigai-devtools' daily scan (`xrust-drift.py`) reads it as
/// THE record and warns when crates.io has a newer xrust. `subset::XRUST_MEASURED_ON` is the
/// crate's own copy, and the test below holds the two equal.
const MEASURED_ON: &str = "2.3.0";

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
            Some(|_: &_| {
                Err(ParseError::MissingNameSpace {
                    prefix: None,
                    nsuri: None,
                    message: None,
                })
            }),
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
    // No key step: since xrust 2.3, `key()` indexes its document lazily on `stctxt`.
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

/// Pin a construct this crate refuses because xrust answers it wrongly: through this crate it
/// is refused as an `InvalidArgument` naming the README `row`; through stock xrust it still
/// gives `wrong`, with no error, where XSLT 1.0 says `correct`.
fn pin_refused(src: &str, templates: &str, wrong: &str, correct: &str, row: &str) {
    assert_ne!(wrong, correct, "a refused case pins a WRONG answer");
    let err = ours(src, templates).expect_err("refused before xrust compiles it");
    assert!(
        err.starts_with("invalid argument `stylesheet`: refused:") && err.contains(row),
        "the refusal names the construct `{row}`: {err}"
    );
    let answer = stock(src, templates).unwrap_or_else(|e| {
        panic!("stock xrust now REFUSES this (`{e}`) by itself: drop the refusal from src/subset.rs and move the row to the refused table in README.md")
    });
    assert!(
        answer != correct,
        "stock xrust's silent defect is FIXED (it now answers the XSLT 1.0 `{correct}`): drop the refusal from src/subset.rs and update README.md and docs/upstream-xrust.md"
    );
    assert_eq!(
        answer, wrong,
        "stock xrust's answer changed but is still not `{correct}`: re-measure and update README.md and docs/upstream-xrust.md"
    );
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

/// The measured version is a RECORD, never a gate: a new xrust release must not turn a PR red
/// (ledger #193). What this checks is that the record is one record: the constant, the README
/// and the upstream drafts name the same release. On another xrust it prints a note; every
/// other test in this file is what fails if an answer moved.
#[test]
fn the_measured_xrust_is_recorded_in_one_place() {
    assert_eq!(
        ikigai_xslt::subset::XRUST_MEASURED_ON,
        MEASURED_ON,
        "the crate's record and this file's (which the daily scan reads) name one release"
    );
    let named = format!("xrust {MEASURED_ON}");
    for (file, text) in [
        ("README.md", include_str!("../README.md")),
        (
            "docs/upstream-xrust.md",
            include_str!("../docs/upstream-xrust.md"),
        ),
    ] {
        assert!(
            text.contains(&format!("**{named}**")),
            "{file} names the measured release as **{named}**, as subset::XRUST_MEASURED_ON does"
        );
    }
    let built = xrust_in_this_build();
    if built != MEASURED_ON {
        eprintln!(
            "note: this build uses xrust {built}, and the subset table was measured on {MEASURED_ON}. \
             If every other test here passes, the table still holds: re-measure, then update \
             subset::XRUST_MEASURED_ON, README.md and docs/upstream-xrust.md together."
        );
    }
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
            "a [position() = n] filter",
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
            "a literal [n] predicate (ignored by xrust 2.2.0, right since 2.3.0)",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="doc/item[2]"/>,<xsl:value-of select="(doc/item)[2]"/>,<xsl:value-of select="doc/item[@k='1'][2]"/>,<xsl:value-of select="count(doc/item[1])"/></out></xsl:template>"#,
            "<out>two,two,three,1</out>",
        ),
        (
            "a literal [n] predicate away from the root, and in an attribute value template",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc"/></out></xsl:template><xsl:template match="doc"><p n="{item[2]}"><xsl:value-of select="item[3]"/></p></xsl:template>"#,
            "<out><p n='two'>three</p></out>",
        ),
        (
            "a computed numeric predicate: [count(../item)] is the last node",
            SRC,
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc"/></out></xsl:template><xsl:template match="doc"><xsl:value-of select="item[count(../item)]"/></xsl:template>"#,
            "<out>three</out>",
        ),
        (
            "attribute value templates over PREFIXED names (empty in xrust 2.2.0, right since 2.3.0)",
            NS_SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="page/l:Item"><a href="{@rdf:about}" t="{concat('x', @rdf:about)}"/></xsl:for-each><xsl:apply-templates select="page"/></out></xsl:template><xsl:template match="page"><b n="{count(l:Item)}" s="{count(l:*)}"/></xsl:template>"#,
            "<out><a href='urn:i:1' t='xurn:i:1'/><a href='urn:i:2' t='xurn:i:2'/><b n='2' s='2'/></out>",
        ),
        (
            "xsl:variable inside a template (refused by xrust 2.2.0)",
            SRC,
            r#"<xsl:template match="/"><xsl:variable name="v" select="doc/@title"/><out><xsl:value-of select="$v"/></out></xsl:template>"#,
            "<out>T</out>",
        ),
        (
            "string-length() (refused by xrust 2.2.0)",
            SRC,
            r#"<xsl:template match="/"><out><xsl:value-of select="string-length(doc/@title)"/>,<xsl:value-of select="string-length('four')"/></out></xsl:template>"#,
            "<out>1,4</out>",
        ),
        (
            "xsl:key over prefixed names (refused by xrust 2.2.0)",
            NS_SRC,
            r#"<xsl:key name="k" match="l:Item" use="@rdf:about"/><xsl:template match="/"><out><xsl:value-of select="count(key('k','urn:i:2'))"/></out></xsl:template>"#,
            "<out>1</out>",
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
            "xsl:sort inside for-each",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:sort select="@id"/><p/></xsl:for-each></out></xsl:template>"#,
            r#"unsupported XSL element "sort""#,
        ),
        (
            "xsl:sort inside for-each-group (newly refused by xrust 2.3.0)",
            SRC,
            r#"<xsl:template match="/"><out><xsl:for-each-group select="doc/item" group-by="@k"><xsl:sort select="@k"/><p/></xsl:for-each-group></out></xsl:template>"#,
            r#"unsupported XSL element "sort""#,
        ),
    ];
    for (what, src, templates, refusal) in cases {
        let err = ours(src, templates).expect_err(what);
        assert!(err.contains(refusal), "{what}: {err}");
    }
}

/// A stylesheet whose first node is a comment, not the `xsl:stylesheet` element: xrust 2.2.0
/// refused it (`not an XSLT stylesheet`), and 2.3.0 compiles it, as XSLT says it should.
#[test]
fn a_leading_comment_before_the_stylesheet_is_admitted() {
    let answer = transform_xml(
        SRC,
        &format!(
            "<!-- a comment -->{}",
            style(r#"<xsl:template match="/"><out/></xsl:template>"#)
        ),
        false,
    );
    assert_eq!(answer.as_deref(), Ok("<out/>"));
}

/// The node-set-to-string defect behind `silent_value_of_a_node_set_concatenates_every_node`
/// is not `xsl:value-of`'s alone: `string-length()` of several nodes measures them all
/// concatenated (`onetwothree`, 11), where XSLT 1.0 measures the first (`one`, 3).
#[test]
fn silent_string_of_a_node_set_concatenates_every_node() {
    pin_silent(
        SRC,
        r#"<xsl:template match="/"><out><xsl:value-of select="string-length(doc/item)"/></out></xsl:template>"#,
        "<out>11</out>",
        "<out>3</out>",
    );
}

// --------------------------------------------------------------------- refused here

/// A predicate in a MATCH pattern is ignored: `item[@k='2']` matches every `item`.
#[test]
fn refused_a_predicate_in_a_match_pattern() {
    pin_refused(
        SRC,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item[@k='2']"><hit><xsl:value-of select="."/></hit></xsl:template>"#,
        "<out><hit>one</hit><hit>two</hit><hit>three</hit></out>",
        "<out>one<hit>two</hit>three</out>",
        "a predicate in a match pattern",
    );
}

/// With two templates for one name, the FIRST declared wins whatever its predicate: XSLT
/// gives `item[@k='1']` priority 0.5 over `item`'s 0, and xrust ignores both the predicate
/// and the priority. Refused as the match predicate it holds.
#[test]
fn refused_template_priority_by_its_predicate() {
    pin_refused(
        SRC,
        r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><other/></xsl:template><xsl:template match="item[@k='1']"><one/></xsl:template>"#,
        "<out><other/><other/><other/></out>",
        "<out><one/><other/><one/></out>",
        "a predicate in a match pattern",
    );
}

/// The predicate in an `xsl:key`'s match pattern is ignored too: every `item` is keyed.
#[test]
fn refused_a_predicate_in_a_key_pattern() {
    pin_refused(
        SRC,
        r#"<xsl:key name="k" match="item[@k='1']" use="@id"/><xsl:template match="/"><out><xsl:value-of select="count(key('k','b'))"/></out></xsl:template>"#,
        "<out>1</out>",
        "<out>0</out>",
        "a predicate in a match pattern",
    );
}

/// `[last()]` selects the FIRST node, since `last()` is 1 there: at the root, from a nested
/// template, on a parenthesized expression and in an attribute value template alike. A literal
/// `[n]` is right since xrust 2.3.0 (see the works table), and so is `[count(../item)]`, the
/// workaround. ⚠ `count()` over the result cannot see this (one node either way): compare values.
#[test]
fn refused_a_last_predicate() {
    for (templates, wrong, correct) in [
        (
            r#"<xsl:template match="/"><out><xsl:value-of select="doc/item[last()]"/></out></xsl:template>"#,
            "<out>one</out>",
            "<out>three</out>",
        ),
        (
            r#"<xsl:template match="/"><out><xsl:value-of select="(doc/item)[last()]"/></out></xsl:template>"#,
            "<out>one</out>",
            "<out>three</out>",
        ),
        (
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc"/></out></xsl:template><xsl:template match="doc"><xsl:value-of select="item[last()]"/></xsl:template>"#,
            "<out>one</out>",
            "<out>three</out>",
        ),
        (
            r#"<xsl:template match="/"><out m="{doc/item[last()]/@id}"/></xsl:template>"#,
            "<out m='a'/>",
            "<out m='c'/>",
        ),
    ] {
        pin_refused(SRC, templates, wrong, correct, "a [last()] predicate");
    }
}

/// `position()` and `last()` are always 1 inside `for-each` and `apply-templates`, so a
/// test such as `position() = 2` is never true and its content is silently absent; and
/// `last()` is 1 inside a predicate too. `position()` inside a predicate works.
#[test]
fn refused_position_and_last_away_from_the_root() {
    pin_refused(
        SRC,
        r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><p><xsl:value-of select="position()"/>/<xsl:value-of select="last()"/></p></xsl:for-each><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><xsl:if test="position() = 2"><second/></xsl:if></xsl:template>"#,
        "<out><p>1/1</p><p>1/1</p><p>1/1</p></out>",
        "<out><p>1/3</p><p>2/3</p><p>3/3</p><second/></out>",
        "position() or last() away from the root",
    );
    pin_refused(
        SRC,
        r#"<xsl:template match="/"><out><xsl:value-of select="count(doc/item[position() &lt; last()])"/></out></xsl:template>"#,
        "<out>0</out>",
        "<out>2</out>",
        "position() or last() away from the root",
    );
}

/// `//name` is evaluated from the CONTEXT node, not the document root, so from inside a
/// nested template it finds only the context's own descendants (usually none) — and the same
/// inside a named template, the body of an `xsl:for-each` and a predicate, wherever they are.
/// `/doc//name` works (see the works table).
#[test]
fn refused_a_leading_double_slash_away_from_the_root() {
    for (templates, wrong, correct) in [
        (
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template><xsl:template match="item"><p><xsl:value-of select="count(//item)"/></p></xsl:template>"#,
            "<out><p>0</p><p>0</p><p>0</p></out>",
            "<out><p>3</p><p>3</p><p>3</p></out>",
        ),
        (
            r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:value-of select="count(//item)"/></xsl:for-each></out></xsl:template>"#,
            "<out>000</out>",
            "<out>333</out>",
        ),
        (
            r#"<xsl:template match="/"><out><xsl:value-of select="count(doc/item[//item])"/></out></xsl:template>"#,
            "<out>0</out>",
            "<out>3</out>",
        ),
    ] {
        pin_refused(
            SRC,
            templates,
            wrong,
            correct,
            "a leading // away from the root",
        );
    }
}

/// Where the context IS the root, the same constructs give the XSLT answer, and are not
/// refused: the scan is conservative.
#[test]
fn the_same_constructs_at_the_root_are_admitted() {
    let cases: &[(&str, &str)] = &[
        (
            r#"<xsl:template match="/"><out><xsl:value-of select="count(//item)"/>,<xsl:value-of select="position()"/>,<xsl:value-of select="last()"/></out></xsl:template>"#,
            "<out>3,1,1</out>",
        ),
        (
            r#"<xsl:variable name="n" select="count(//item)"/><xsl:template match="/"><out><xsl:apply-templates select="//item[@k='2']"/></out></xsl:template><xsl:template match="item"><xsl:value-of select="."/></xsl:template>"#,
            "<out>two</out>",
        ),
        (
            r#"<xsl:template match="/"><out><xsl:apply-templates select="doc/item[position() = 2]"/></out></xsl:template><xsl:template match="item"><p t="{concat('[1]', '//', 'rdf:about')}"><xsl:value-of select="'item[1]'"/></p></xsl:template>"#,
            "<out><p t='[1]//rdf:about'>item[1]</p></out>",
        ),
    ];
    for (templates, expected) in cases {
        assert_eq!(
            ours(SRC, templates).as_deref(),
            Ok(*expected),
            "{templates}"
        );
    }
}

/// Through the endpoint the refusal is TYPED: an `InvalidArgument` naming `stylesheet`, as
/// a bound is, so a caller can tell a stylesheet it must change from a transform that failed.
#[test]
fn the_endpoint_refuses_with_a_typed_invalid_argument() {
    use futures::executor::block_on;
    use ikigai_core::{ArgRef, Capability, Error as IkError, Iri, Kernel, Request, Verb};
    use std::sync::Arc;

    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(SRC.as_bytes().to_vec()))
        .with_arg(
            "stylesheet",
            ArgRef::Inline(
                style(r#"<xsl:template match="item[1]"><x/></xsl:template>"#).into_bytes(),
            ),
        );
    let err = block_on(kernel.issue(req, &Capability::root())).expect_err("refused");
    assert!(
        matches!(&err, IkError::InvalidArgument { name, detail }
            if name == "stylesheet" && detail.contains("a predicate in a match pattern")),
        "{err}"
    );
}

// --------------------------------------------------------------------------- silent

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

/// `xsl:output method="html"`: stock xrust serializes XML, so every empty element is
/// self-closed and a browser reads `<script …/>` as an OPEN script element that swallows the
/// rest of the page. Since 0.3.0 this crate serializes HTML (`tests/html_output.rs` has the
/// rest); this pins that the gap is still xrust's, so the crate's serialization still earns
/// its keep.
#[test]
fn html_output_is_html_here_and_xml_in_stock_xrust() {
    let templates = r#"<xsl:template match="/"><html><head><script src="a.js"></script></head><body><textarea></textarea><br/></body></html></xsl:template>"#;
    let html = |t: &str| style(t).replace(r#"method="xml""#, r#"method="html""#);
    assert_eq!(
        transform_xml(SRC, &html(templates), false).as_deref(),
        Ok("<html><head><script src='a.js'></script></head><body><textarea></textarea><br/></body></html>")
    );
    // Stock xrust, through the same harness: XML whatever the method says.
    let stock_html = {
        let doc = |s: &str| {
            let d = RNode::new_document();
            parse(
                d.clone(),
                s,
                Some(|_: &_| {
                    Err(ParseError::MissingNameSpace {
                        prefix: None,
                        nsuri: None,
                        message: None,
                    })
                }),
            )
            .map(|_| d)
        };
        let src = doc(SRC).unwrap();
        let mut stctxt = StaticContextBuilder::new()
            .message(|_| Ok(()))
            .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "no fetcher")))
            .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "no parser")))
            .build();
        let mut ctxt = from_document(doc(&html(templates)).unwrap(), None, doc, |_| {
            Ok(String::new())
        })
        .unwrap();
        ctxt.context(vec![Item::Node(src.clone())], 0);
        ctxt.result_document(RNode::new_document());
        ctxt.evaluate(&mut stctxt).unwrap().to_xml()
    };
    assert_eq!(
        stock_html, "<html><head><script src='a.js'/></head><body><textarea/><br/></body></html>",
        "stock xrust's serialization changed: if it now writes HTML for method=\"html\", \
         src/html.rs may be redundant; update the README and docs/upstream-xrust.md"
    );
}

/// Ledger #2146: stylesheet whitespace stripping also removes xsl:text's explicit space.
/// Keep this measured limitation visible until xrust preserves it; the workaround below
/// pins a useful alternative without changing the caller's stylesheet here.
#[test]
fn silent_whitespace_only_xsl_text_is_dropped() {
    for text in [" ", "  ", "\t\n", "&#x20;"] {
        let templates = format!(
            r#"<xsl:template match="/"><out>Meta<xsl:text>{text}</xsl:text>Source</out></xsl:template>"#
        );
        let preserved = if text == "&#x20;" { " " } else { text };
        pin_silent(
            "<doc/>",
            &templates,
            "<out>MetaSource</out>",
            &format!("<out>Meta{preserved}Source</out>"),
        );
    }
}

#[test]
fn whitespace_xsl_text_workaround_and_nonblank_text_are_preserved() {
    for templates in [
        r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:value-of select="concat(., ' ')"/></xsl:for-each></out></xsl:template>"#,
        r#"<xsl:template match="/"><out><xsl:text>Meta </xsl:text><xsl:text>Source </xsl:text></out></xsl:template>"#,
        r#"<xsl:template match="/"><out>Meta<xsl:value-of select="' '"/>Source<xsl:value-of select="' '"/></out></xsl:template>"#,
    ] {
        for answer in [
            ours("<doc><item>Meta</item><item>Source</item></doc>", templates),
            stock("<doc><item>Meta</item><item>Source</item></doc>", templates),
        ] {
            assert_eq!(answer.unwrap(), "<out>Meta Source </out>");
        }
    }
}

/// Ledger #2519: unprefixed input values retain their case.
#[test]
fn unprefixed_attribute_values_keep_their_case() {
    let src = r#"<doc plain="urn:I:One"/>"#;
    let templates =
        r#"<xsl:template match="/"><out><xsl:value-of select="doc/@plain"/></out></xsl:template>"#;
    for answer in [ours(src, templates), stock(src, templates)] {
        assert_eq!(answer.unwrap(), "<out>urn:I:One</out>");
    }
}

fn assert_case_refusal(error: &str, arg: &str) {
    assert!(
        error.starts_with(&format!("invalid argument `{arg}`: refused:")),
        "{error}"
    );
    for detail in [
        "prefixed attribute",
        "xrust",
        "lowercase",
        "#2519",
        "compute the value from data",
    ] {
        assert!(
            error.contains(detail),
            "refusal must name {detail}: {error}"
        );
    }
}

#[test]
fn refused_prefixed_attribute_case_in_both_inputs() {
    let cases = [
        (
            r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="https://Example.org/My-Post"/></rdf:RDF>"#,
            r#"<xsl:template match="/"><out><xsl:value-of select="/rdf:RDF/rdf:Description/@rdf:about"/></out></xsl:template>"#,
            "<out>https://example.org/my-post</out>",
            "<out>https://Example.org/My-Post</out>",
        ),
        (
            "<doc/>",
            r#"<xsl:template match="/"><rdf:v rdf:datatype="http://www.w3.org/2001/XMLSchema#dateTime"/></xsl:template>"#,
            "<rdf:v xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#' rdf:datatype='http://www.w3.org/2001/xmlschema#datetime'/>",
            "<rdf:v xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#' rdf:datatype='http://www.w3.org/2001/XMLSchema#dateTime'/>",
        ),
    ];
    for (i, (src, templates, wrong, correct)) in cases.into_iter().enumerate() {
        let answer = stock(src, templates).unwrap();
        assert_ne!(
            answer, correct,
            "xrust fixed #2519: remove the guard and update the README and upstream draft"
        );
        assert_eq!(answer, wrong, "xrust's answer changed: re-measure #2519");
        let error = ours(src, templates).expect_err("refuse before parsing");
        assert_case_refusal(&error, if i == 0 { "src" } else { "stylesheet" });
        assert!(
            error.contains(if i == 0 { "rdf:about" } else { "rdf:datatype" }),
            "{error}"
        );
    }
}

#[test]
fn refused_encoded_and_unicode_prefixed_values_in_either_input() {
    let templates = r#"<xsl:template match="/"><out/></xsl:template>"#;
    for value in ["My-Post", "&#77;y-&#x50;ost", "Été", "&#x130;", "&iri;"] {
        let src = format!(
            r#"<!DOCTYPE doc [<!ENTITY iri "My-Post">]><doc xmlns:f="urn:f" f:k = '{value}'/>"#
        );
        assert_case_refusal(&ours(&src, templates).unwrap_err(), "src");
        let stylesheet = format!(
            r#"<!DOCTYPE xsl:stylesheet [<!ENTITY iri "My-Post">]>{}"#,
            style(&format!(
                r#"<xsl:template match="/"><out xmlns:f="urn:f" f:k = '{value}'/></xsl:template>"#
            ))
        );
        assert_case_refusal(
            &transform_xml("<doc/>", &stylesheet, false).unwrap_err(),
            "stylesheet",
        );
    }
    for name in ["xml:id", "xml:base", "xml:space"] {
        let src = format!(r#"<doc {name}="Upper"/>"#);
        let error = ours(&src, templates).unwrap_err();
        assert_case_refusal(&error, "src");
        assert!(error.contains(name), "{error}");
    }
}

/// `xml:lang` is case-insensitive (BCP 47), so a literal one is admitted in both inputs even
/// though xrust lowercases it: `en-us` means what `en-US` meant. A value template there is still
/// refused, because xrust would lowercase the variable names inside it.
#[test]
fn a_literal_xml_lang_is_admitted_and_reads_back_lowercased() {
    let read = r#"<xsl:template match="/"><out><xsl:value-of select="doc/@xml:lang"/></out></xsl:template>"#;
    assert_eq!(
        ours(r#"<doc xml:lang="en-US"/>"#, read).unwrap(),
        "<out>en-us</out>"
    );
    let literal = style(r#"<xsl:template match="/"><out xml:lang="en-US"/></xsl:template>"#);
    assert!(transform_xml("<doc/>", &literal, false).is_ok());
    let avt = r#"<xsl:template match="/"><xsl:variable name="Lang" select="'en'"/><out xml:lang="{$Lang}"/></xsl:template>"#;
    assert_case_refusal(&ours("<doc/>", avt).unwrap_err(), "stylesheet");
}

#[test]
fn lowercase_values_and_computed_values_remain_admitted() {
    // Uppercase namespace URIs, entity NAMES, text, unprefixed values, and quoted markup
    // in comments/CDATA/PIs must not be mistaken for a damaged attribute value.
    let src = r#"<!DOCTYPE doc [<!ENTITY UpperName "lower">]>
<?p f:k="Upper"?><doc xmlns:f="urn:Upper" xmlns:l="urn:l#" l:k="&UpperName;" plain="My-Post"><!-- f:k="Upper" --><![CDATA[f:k="Upper"]]><iri>My-Post</iri></doc>"#;
    assert_eq!(
        ours(
            src,
            r#"<xsl:template match="/"><out><xsl:value-of select="doc/@l:k"/></out></xsl:template>"#
        )
        .unwrap(),
        "<out>lower</out>"
    );

    let templates = r#"<xsl:template match="/"><xsl:variable name="iri" select="doc/iri"/><rdf:v rdf:about="{$iri}" rdf:datatype="{doc/@datatype}"/></xsl:template>"#;
    let src = r#"<doc datatype="http://www.w3.org/2001/XMLSchema#dateTime"><iri>https://Example.org/My-Post</iri></doc>"#;
    assert_eq!(ours(src, templates).unwrap(), "<rdf:v xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#' rdf:about='https://Example.org/My-Post' rdf:datatype='http://www.w3.org/2001/XMLSchema#dateTime'/>");
}

#[test]
fn refused_prefixed_avt_before_its_variable_name_is_lowercased() {
    let templates = r#"<xsl:template match="/"><xsl:variable name="camelVar" select="'My-Post'"/><rdf:v rdf:about="{$camelVar}"/></xsl:template>"#;
    let error = stock("<doc/>", templates).unwrap_err();
    assert!(error.contains("camelvar"), "{error}");
    assert_case_refusal(&ours("<doc/>", templates).unwrap_err(), "stylesheet");
}

#[test]
fn the_compiled_api_refuses_prefixed_values_in_both_inputs() {
    let bad_style = style(r#"<xsl:template match="/"><rdf:v rdf:about="My-Post"/></xsl:template>"#);
    let error = ikigai_xslt::CompiledStylesheet::compile(&bad_style)
        .err()
        .expect("refused");
    assert_case_refusal(&error, "stylesheet");
    let result = ikigai_xslt::limits::on_xslt_stack(|| {
        let compiled = ikigai_xslt::CompiledStylesheet::compile(&style(
            r#"<xsl:template match="/"><out/></xsl:template>"#,
        ))?;
        compiled.transform(r#"<doc xmlns:f="urn:f" f:k="My-Post"/>"#, false)
    })
    .unwrap();
    assert_case_refusal(&result.unwrap_err(), "src");
}

#[test]
fn prefixed_case_refusals_at_the_endpoint_are_typed() {
    use futures::executor::block_on;
    use ikigai_core::{ArgRef, Capability, Error as IkError, Iri, Kernel, Request, Verb};
    use std::sync::Arc;
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for (arg, src, templates) in [
        (
            "src",
            r#"<doc xmlns:f="urn:f" f:k="My-Post"/>"#,
            r#"<xsl:template match="/"><out/></xsl:template>"#,
        ),
        (
            "content",
            r#"<doc xmlns:f="urn:f" f:k="My-Post"/>"#,
            r#"<xsl:template match="/"><out/></xsl:template>"#,
        ),
        (
            "stylesheet",
            "<doc/>",
            r#"<xsl:template match="/"><rdf:v rdf:about="My-Post"/></xsl:template>"#,
        ),
    ] {
        let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
            .with_arg(
                if arg == "content" { "content" } else { "src" },
                ArgRef::Inline(src.as_bytes().to_vec()),
            )
            .with_arg("stylesheet", ArgRef::Inline(style(templates).into_bytes()));
        let error = block_on(kernel.issue(request, &Capability::root())).expect_err("refused");
        assert!(
            matches!(&error, IkError::InvalidArgument { name, detail }
            if name == arg && detail.contains("#2519")),
            "{error}"
        );
    }
}

// Ledger #2520: preserve the distinction between literal whitespace and references.
#[test]
fn refused_attribute_whitespace_character_references() {
    for reference in ["&#10;", "&#xA;", "&#x0a;", "&#00010;", "&#9;", "&#xD;"] {
        let templates = format!(
            r#"<xsl:template match="/"><out><xsl:value-of select="concat('a', '{reference}')"/></out></xsl:template>"#
        );
        assert_eq!(stock(SRC, &templates).unwrap(), "<out>a </out>");
        let error = ours(SRC, &templates).unwrap_err();
        for detail in [
            "invalid argument `stylesheet`",
            "character reference",
            "space",
            "#2520",
            "lf-holder",
            "substring",
        ] {
            assert!(error.contains(detail), "{error}");
        }
        let src = format!(r#"<doc v="a{reference}b"/>"#);
        let read =
            r#"<xsl:template match="/"><out><xsl:value-of select="doc/@v"/></out></xsl:template>"#;
        assert_eq!(stock(&src, read).unwrap(), "<out>a b</out>");
        let error = ours(&src, read).unwrap_err();
        assert!(
            error.starts_with("invalid argument `src`: refused: character reference"),
            "{error}"
        );
    }
}

#[test]
fn newline_workaround_and_reference_boundaries() {
    let templates = "<xsl:template match=\"/\"><xsl:variable name=\"lf-holder\">.\n.</xsl:variable><out><xsl:for-each select=\"doc/item\"><xsl:value-of select=\"concat(., substring($lf-holder, 2, 1))\"/></xsl:for-each></out></xsl:template>";
    assert_eq!(
        ours(SRC, templates).unwrap(),
        "<out>one\ntwo\nthree\n</out>"
    );
    assert_eq!(
        stock(SRC, templates).unwrap(),
        "<out>one\ntwo\nthree\n</out>"
    );
    let text = style(templates).replace(r#"method="xml""#, r#"method="text""#);
    assert_eq!(
        transform_xml(SRC, &text, true).unwrap(),
        "one\ntwo\nthree\n"
    );
    // Numeric references in text are preserved; escaped ampersands aren't references.
    let src = "<?p v='&#10;'?><doc v='&amp;#10;'><!-- v='&#10;' --><![CDATA[&#10;]]>&#10;</doc>";
    let t = r#"<xsl:template match="/"><out><xsl:value-of select="doc/@v"/></out></xsl:template>"#;
    assert_eq!(ours(src, t).unwrap(), "<out>&amp;#10;</out>");
    let src = "<doc v='a\nb'/>";
    assert_eq!(ours(src, t).unwrap(), "<out>a b</out>");
}

#[test]
fn boolean_value_of_is_corrected_here() {
    for (expr, expected) in [
        ("1 &gt; 0", "true"),
        ("1 &lt; 0", "false"),
        ("true()", "true"),
        ("false()", "false"),
        ("boolean(doc/item)", "true"),
        ("not(doc/item)", "false"),
        ("contains('abc', 'b')", "true"),
        ("starts-with('abc', 'z')", "false"),
        ("(true() and false()) or true()", "true"),
        ("doc/item = 'two'", "true"),
    ] {
        let templates = format!(
            r#"<xsl:template match="/"><out><xsl:value-of select="{expr}"/></out></xsl:template>"#
        );
        assert_eq!(
            stock(SRC, &templates).unwrap(),
            "<out></out>",
            "stock boolean conversion changed: remeasure #2520"
        );
        assert_eq!(
            ours(SRC, &templates).unwrap(),
            format!("<out>{expected}</out>"),
            "{expr}"
        );
    }
    let templates = r#"<xsl:variable name="v" select="false()"/><xsl:template match="/"><out><xsl:value-of select="$v"/><xsl:variable name="v" select="true()"/><xsl:value-of select="$v"/></out></xsl:template>"#;
    assert_eq!(stock(SRC, templates).unwrap(), "<out></out>");
    assert_eq!(ours(SRC, templates).unwrap(), "<out>falsetrue</out>");
    let nested = r#"<xsl:template match="/"><out><xsl:for-each select="doc/item"><xsl:value-of select="@k = '1'"/></xsl:for-each></out></xsl:template>"#;
    assert_eq!(ours(SRC, nested).unwrap(), "<out>truefalsetrue</out>");
    // A comparison in a predicate or string is not the value-of result type.
    let t = r#"<xsl:template match="/"><out><xsl:value-of select="doc/item[@k='2']"/><xsl:value-of select="'1 > 0'"/><xsl:value-of select="count(doc/item)"/></out></xsl:template>"#;
    assert_eq!(ours(SRC, t).unwrap(), "<out>two1 &gt; 03</out>");
}

#[test]
fn boolean_parameter_value_of_is_refused_when_its_type_can_change() {
    let t = r#"<xsl:param name="v" select="false()"/><xsl:template match="/"><out><xsl:value-of select="$v"/></out></xsl:template>"#;
    let error = ours(SRC, t).unwrap_err();
    assert!(
        error.starts_with("invalid argument `stylesheet`: refused:"),
        "{error}"
    );
    assert!(
        error.contains("boolean") && error.contains("xsl:choose"),
        "{error}"
    );
}

#[test]
fn refused_literal_attribute_without_result_namespace() {
    for templates in [
        r#"<xsl:template match="/"><out><b rdf:datatype="x">1</b></out></xsl:template>"#,
        r#"<xsl:template match="/"><out><b xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" rdf:datatype="x">1</b></out></xsl:template>"#,
    ] {
        let panic = std::panic::catch_unwind(|| stock(SRC, templates))
            .expect_err("stock xrust must still panic");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(message.contains("unable to find namespace"), "{message}");
        let error = ours(SRC, templates).unwrap_err();
        assert!(
            error.starts_with("invalid argument `stylesheet`: refused:"),
            "{error}"
        );
        assert!(
            error.contains("a literal attribute without a result namespace declaration"),
            "{error}"
        );
    }
    let t =
        r#"<xsl:template match="/"><rdf:RDF><b rdf:datatype="x">1</b></rdf:RDF></xsl:template>"#;
    assert_eq!(ours(SRC, t).unwrap(), stock(SRC, t).unwrap());
}

#[test]
fn refusals_2520_are_typed_at_the_endpoint() {
    use futures::executor::block_on;
    use ikigai_core::{ArgRef, Capability, Error as IkError, Iri, Kernel, Request, Verb};
    use std::sync::Arc;
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    for (arg, src, templates, detail) in [
        (
            "src",
            r#"<doc v="&#10;"/>"#,
            r#"<xsl:template match="/"><out/></xsl:template>"#,
            "character reference",
        ),
        (
            "content",
            r#"<doc v="&#10;"/>"#,
            r#"<xsl:template match="/"><out/></xsl:template>"#,
            "character reference",
        ),
        (
            "stylesheet",
            SRC,
            r#"<xsl:template match="/"><xsl:value-of select="'&#10;'"/></xsl:template>"#,
            "character reference",
        ),
        (
            "stylesheet",
            SRC,
            r#"<xsl:template match="/"><out rdf:datatype="x"/></xsl:template>"#,
            "a literal attribute without a result namespace declaration",
        ),
        (
            "stylesheet",
            SRC,
            r#"<xsl:param name="v" select="true()"/><xsl:template match="/"><xsl:value-of select="$v"/></xsl:template>"#,
            "boolean",
        ),
    ] {
        let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
            .with_arg(
                if arg == "content" { "content" } else { "src" },
                ArgRef::Inline(src.as_bytes().to_vec()),
            )
            .with_arg("stylesheet", ArgRef::Inline(style(templates).into_bytes()));
        let error = block_on(kernel.issue(request, &Capability::root())).unwrap_err();
        assert!(
            matches!(&error, IkError::InvalidArgument { name, detail: d } if name == arg && d.contains(detail)),
            "{error}"
        );
    }
}
