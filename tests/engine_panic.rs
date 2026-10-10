//! xrust panics on some input a caller controls; the caller gets an error, never the panic
//! (ledger #1040).
//!
//! The claim: `<xsl:copy-of select="/"/>` makes xrust 2.2.0 panic ("unable to attach to
//! result document"), and `on_xslt_stack` re-raised it on the caller's thread — a request
//! handler, in a host. Reproduced before the fix: the first test below failed with the panic
//! on the caller's thread. Probing the same path turned up two families, both now answered as
//! `Error::Endpoint`, through the endpoint and the library entry point alike:
//!
//! - **attaching to the result document** (`transform/context.rs`): a document node or an
//!   attribute at the top of the result — `copy-of`/`sequence` of `/` or of an attribute,
//!   `xsl:copy` with the document node as context, `xsl:attribute` outside any element;
//! - **an `xsl:sort` key that fails to evaluate** (`transform/mod.rs`, `controlflow.rs`) —
//!   an unknown variable or function — under `for-each`, `apply-templates` and
//!   `for-each-group`.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Error, Iri, Kernel, Request, Verb};
use std::sync::Arc;

fn style(body: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/">{body}</xsl:template>
</xsl:stylesheet>"#
    )
}

const SRC: &str = r#"<doc a="1"><x k="3">t</x><x k="1">u</x></doc>"#;

/// Every body xrust 2.2.0 panicked on, with a word its message carries.
const PANICS: [(&str, &str); 9] = [
    (r#"<xsl:copy-of select="/"/>"#, "attach"),
    (r#"<xsl:copy-of select="//@*"/>"#, "attach"),
    (r#"<xsl:sequence select="/"/>"#, "attach"),
    (r#"<xsl:copy/>"#, "attach"),
    (r#"<xsl:attribute name="a">1</xsl:attribute>"#, "attach"),
    (
        r#"<xsl:for-each select="//x"><xsl:sort select="$nope"/><xsl:value-of select="."/></xsl:for-each>"#,
        "key value",
    ),
    (
        r#"<xsl:for-each select="//x"><xsl:sort select="nosuch(.)"/><xsl:value-of select="."/></xsl:for-each>"#,
        "key value",
    ),
    (
        r#"<xsl:apply-templates select="//x"><xsl:sort select="$nope"/></xsl:apply-templates>"#,
        "key value",
    ),
    (
        r#"<xsl:for-each-group select="//x" group-by="@k"><xsl:sort select="$nope"/><xsl:value-of select="."/></xsl:for-each-group>"#,
        "key value",
    ),
];

fn transform(src: &str, style: &str) -> Result<String, Error> {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(src.as_bytes().to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(style.as_bytes().to_vec()));
    block_on(kernel.issue(req, &Capability::root()))
        .map(|rep| String::from_utf8_lossy(&rep.bytes).into_owned())
}

#[test]
fn copying_the_document_node_is_an_error_not_a_panic() {
    let caught = std::panic::catch_unwind(|| transform("<doc><a/></doc>", &style(PANICS[0].0)));
    let answer = caught.expect("the transform panicked on the caller's thread");
    let err = answer.expect_err("xrust cannot run it, so it is refused");
    assert!(matches!(&err, Error::Endpoint(_)), "{err:?}");
    let text = err.to_string();
    assert!(
        text.contains("panicked") && text.contains("unable to attach to result document"),
        "the refusal names the cause: {text}"
    );
}

#[test]
fn every_panic_found_is_an_endpoint_error_through_the_endpoint() {
    for (body, word) in PANICS {
        let caught = std::panic::catch_unwind(|| transform(SRC, &style(body)));
        let err = caught
            .unwrap_or_else(|_| panic!("{body}: panicked on the caller's thread"))
            .expect_err(body);
        assert!(matches!(&err, Error::Endpoint(_)), "{body}: {err:?}");
        assert!(err.to_string().contains(word), "{body}: {err}");
    }
}

#[test]
fn the_library_entry_point_answers_an_error_too() {
    for (body, word) in PANICS {
        let caught =
            std::panic::catch_unwind(|| ikigai_xslt::transform_xml(SRC, &style(body), false));
        let err = caught
            .unwrap_or_else(|_| panic!("{body}: panicked on the caller's thread"))
            .expect_err(body);
        assert!(
            err.starts_with("endpoint error:") && err.contains(word),
            "{body}: {err}"
        );
    }
}

#[test]
fn a_transform_after_a_panic_still_answers() {
    // A panicked pool thread is dropped rather than given back; the next call gets another.
    for _ in 0..3 {
        assert!(transform(SRC, &style(PANICS[0].0)).is_err());
        let ok = transform(SRC, &style(r#"<r><xsl:value-of select="count(//x)"/></r>"#)).unwrap();
        assert_eq!(ok, "<r>2</r>");
    }
}
