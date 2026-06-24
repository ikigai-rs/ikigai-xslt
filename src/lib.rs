//! `ikigai-xslt` — XSLT transformation as an ikigai resource.
//!
//! `urn:xslt:transform?src=<uri>&stylesheet=<uri>` applies an XSLT `stylesheet` to a
//! `src` document — **both resolved through the kernel as cacheable resource
//! references**. Because each is fetched with `inv.source`/`inv.issue`, the result
//! depends on both golden threads, so it is `.cacheable()` and auto-invalidates when
//! either the source or the stylesheet changes. `src` may instead be piped in (so it
//! also composes in a pipeline, e.g. `… | urn:rdf:transrept as=application/rdf+xml |
//! urn:xslt:transform stylesheet=<uri>`).
//!
//! This is a general styling mechanism for arbitrary XML — RDF/XML in particular — so
//! the same cached graph can be rendered into different presentations by swapping the
//! stylesheet. Built on `xrust` (pure-Rust XPath 1.0 / XSLT 1.0), so it runs natively
//! and in the browser (wasm) alike.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use ikigai_core::{
    ArgRef, ArgSpec, Description, Endpoint, EndpointSpace, Error, Exact, Invocation, Iri, ReprType,
    Representation, Request, Result, Verb,
};
use xrust::item::{Item, Node, SequenceTrait};
use xrust::parser::xml::parse as xmlparse;
use xrust::parser::ParseError;
use xrust::transform::context::StaticContextBuilder;
use xrust::trees::smite::RNode;
use xrust::xdmerror::{Error as XsltError, ErrorKind as XsltErrorKind};
use xrust::xslt::from_document;

/// Bind `urn:xslt:transform`. Mount this space in a host kernel's root.
pub fn space() -> EndpointSpace {
    EndpointSpace::new().bind(Exact::new("urn:xslt:transform"), XsltEndpoint)
}

struct XsltEndpoint;

#[async_trait]
impl Endpoint for XsltEndpoint {
    async fn invoke(&self, inv: &Invocation<'_>) -> Result<Representation> {
        // The stylesheet — always a resolvable resource reference.
        let style_uri = inv.inline_str("stylesheet").map_err(|_| {
            Error::Endpoint(
                "urn:xslt:transform needs a `stylesheet=<uri>` resource reference".to_string(),
            )
        })?;
        let stylesheet = utf8(resolve_ref(inv, style_uri).await?, "stylesheet")?;

        // The source document. `src` is either a resource IRI to resolve, or — when the
        // document is piped in (the engine routes a piped value to the first input) — the
        // inline XML itself. XML always starts with `<`, an IRI never does, so that's the
        // discriminator. An explicit `content=` is also accepted.
        let source = if let Ok(src) = inv.inline_str("src") {
            if src.trim_start().starts_with('<') {
                src.to_string()
            } else {
                utf8(resolve_ref(inv, src).await?, "src")?
            }
        } else if let Ok(content) = inv.inline_str("content") {
            content.to_string()
        } else {
            return Err(Error::Endpoint(
                "urn:xslt:transform needs a `src=<uri>` resource reference (or a piped document)"
                    .to_string(),
            ));
        };

        // The output media type — default text/html (styling RDF/XML into a page).
        let media = inv.inline_str("as").unwrap_or("text/html").to_string();
        // `text/plain` output is a `method="text"` stylesheet: serialize the result's
        // string value (whitespace preserved). Anything else is markup → XML serialize.
        let text_output = media.split(';').next().unwrap_or(&media).trim() == "text/plain";

        // Transform synchronously — xrust's tree types never cross an `await`, so the
        // endpoint future stays `Send`. Cacheable — the result inherits the src +
        // stylesheet threads.
        let out = transform(&source, &stylesheet, text_output)?;
        Ok(Representation::new(
            ReprType::new(media).with_param("charset", "utf-8"),
            out.into_bytes(),
        )
        .cacheable())
    }

    fn name(&self) -> &str {
        "xslt-transform"
    }

    fn describe(&self) -> Description {
        Description::new("xslt-transform")
            .title("XSLT transform")
            .summary(
                "Apply an XSLT stylesheet to a source document — both as cacheable resource \
                 references — to style arbitrary XML (e.g. RDF/XML) into HTML.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(ArgSpec::new("src").summary(
                "the source XML/RDF-XML document: a resolvable resource IRI (or pipe it in)",
            ))
            .input(
                ArgSpec::new("stylesheet").summary("the XSLT stylesheet: a resolvable resource IRI"),
            )
            .input(ArgSpec::new("as").summary("output media type (default text/html)"))
            .output("text/html;charset=utf-8")
    }
}

/// Resolve a resource reference through the kernel. An `http(s)://` URL is fetched via
/// the HTTP module (`urn:httpGet`); a `urn:`/`file:` IRI resolves directly. Either way
/// the kernel records the source's golden thread, so the transform is cacheable and
/// invalidates when the referenced resource changes.
async fn resolve_ref(inv: &Invocation<'_>, uri: &str) -> Result<Representation> {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        let get = Iri::parse("urn:httpGet").expect("urn:httpGet is a valid IRI");
        let request =
            Request::new(Verb::Source, get).with_arg("url", ArgRef::Inline(uri.as_bytes().to_vec()));
        inv.issue(request).await
    } else {
        let iri =
            Iri::parse(uri).map_err(|e| Error::Endpoint(format!("bad resource IRI `{uri}`: {e}")))?;
        inv.source(&iri).await
    }
}

/// Decode a resolved representation as UTF-8 text, naming the role in any error.
fn utf8(repr: Representation, role: &str) -> Result<String> {
    String::from_utf8(repr.bytes)
        .map_err(|e| Error::Endpoint(format!("{role} is not valid UTF-8: {e}")))
}

/// The synchronous XSLT transform — the crate's public, host-agnostic entry point.
/// Parses `src_xml` and `stylesheet_xml`, applies the stylesheet, and serializes the
/// result: as its string value when `text_output` (a `method="text"` stylesheet,
/// whitespace preserved), otherwise as XML/markup. Errors are returned as plain
/// strings so the function carries no ikigai-core types — which lets a standalone
/// **wasm module** wrapper expose it directly. (The endpoint above wraps it.)
pub fn transform_xml(
    src_xml: &str,
    stylesheet_xml: &str,
    text_output: bool,
) -> std::result::Result<String, String> {
    let srcdoc =
        parse_xml(src_xml).map_err(|e| format!("source document parse error: {}", e.message))?;
    let styledoc =
        parse_xml(stylesheet_xml).map_err(|e| format!("stylesheet parse error: {}", e.message))?;

    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| {
            Err(XsltError::new(
                XsltErrorKind::NotImplemented,
                "document() fetching is not supported".to_string(),
            ))
        })
        .parser(|_| {
            Err(XsltError::new(
                XsltErrorKind::NotImplemented,
                "runtime parsing is not supported".to_string(),
            ))
        })
        .build();

    let mut ctxt = from_document(styledoc, None, parse_xml, |_| Ok(String::new()))
        .map_err(|e| format!("stylesheet compile error: {}", e.message))?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    ctxt.result_document(RNode::new_document());
    ctxt.populate_key_values(&mut stctxt, srcdoc.clone())
        .map_err(|e| format!("xsl:key error: {}", e.message))?;
    let seq = ctxt
        .evaluate(&mut stctxt)
        .map_err(|e| format!("transform error: {}", e.message))?;
    Ok(if text_output {
        seq.to_string()
    } else {
        seq.to_xml()
    })
}

/// Endpoint-facing wrapper: the public [`transform_xml`] mapped into an ikigai error.
fn transform(src_xml: &str, stylesheet_xml: &str, text_output: bool) -> Result<String> {
    transform_xml(src_xml, stylesheet_xml, text_output).map_err(Error::Endpoint)
}

/// Parse an XML string into an `xrust` document tree.
fn parse_xml(s: &str) -> std::result::Result<RNode, XsltError> {
    let doc = RNode::new_document();
    xmlparse(doc.clone(), s, Some(|_: &_| Err(ParseError::MissingNameSpace)))?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_xml_through_a_stylesheet() {
        // A minimal "select values into a template" transform — the shape the catalog
        // cards use, in miniature.
        let src = "<doc><item>hello</item><item>world</item></doc>";
        let style = r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
            <xsl:template match='/'><ul><xsl:apply-templates select='doc/item'/></ul></xsl:template>
            <xsl:template match='item'><li class='card'><xsl:value-of select='.'/></li></xsl:template>
        </xsl:stylesheet>"#;
        let out = transform(src, style, false).expect("transform");
        assert!(
            out.contains("<li class=\"card\">") || out.contains("<li class='card'>"),
            "got: {out}"
        );
        assert!(out.contains("hello") && out.contains("world"), "got: {out}");
    }

    #[test]
    fn reports_a_stylesheet_error() {
        let err = transform("<a/>", "not a stylesheet", false).unwrap_err();
        assert!(matches!(err, Error::Endpoint(_)));
    }

    /// De-risk the catalog-cards use case: namespaced RDF/XML (the exact shape oxrdf
    /// emits for `urn:kernel:catalog`) → one styled card per `ik:Endpoint`, selecting
    /// title/id/summary/verb/output. Proves xrust handles the default-namespace-per-
    /// element RDF/XML plus descendant matching and `for-each`.
    #[test]
    fn renders_endpoint_cards_from_catalog_rdfxml() {
        let rdfxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <Endpoint xmlns="https://ikigai-rs.dev/ns#" rdf:about="urn:ikigai:endpoint:toUpper">
    <id>toUpper</id><title>Upper-case</title>
    <summary>Upper-cases the text.</summary>
    <verb>Source</verb><verb>Meta</verb>
    <output>text/plain;charset=utf-8</output>
  </Endpoint>
  <rdf:Description rdf:about="urn:ikigai:endpoint:toUpper">
    <input xmlns="https://ikigai-rs.dev/ns#" rdf:nodeID="b0"/>
  </rdf:Description>
  <Endpoint xmlns="https://ikigai-rs.dev/ns#" rdf:about="urn:ikigai:endpoint:reverseList">
    <id>reverseList</id><title>Reverse list</title>
    <summary>Reverses the items.</summary>
    <verb>Source</verb>
    <output>text/plain;charset=utf-8</output>
  </Endpoint>
</rdf:RDF>"#;
        let xsl = r#"<xsl:stylesheet version="1.0"
  xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:ik="https://ikigai-rs.dev/ns#">
  <xsl:template match="/">
    <div class="cat-cards"><xsl:apply-templates select="//ik:Endpoint"/></div>
  </xsl:template>
  <xsl:template match="ik:Endpoint">
    <div class="cat-card">
      <h3 class="cat-title"><xsl:value-of select="ik:title"/></h3>
      <code class="cat-id"><xsl:value-of select="ik:id"/></code>
      <p class="cat-summary"><xsl:value-of select="ik:summary"/></p>
      <xsl:for-each select="ik:verb"><span class="cat-verb"><xsl:value-of select="."/></span></xsl:for-each>
    </div>
  </xsl:template>
</xsl:stylesheet>"#;
        let out = transform(rdfxml, xsl, false).expect("transform");
        // Two cards, one per endpoint, with their titles and ids. (Match the full class
        // attribute so the `cat-cards` wrapper isn't counted as a `cat-card`; xrust emits
        // single-quoted attributes.)
        let cards = out.matches("'cat-card'").count() + out.matches("\"cat-card\"").count();
        assert_eq!(cards, 2, "two cards: {out}");
        assert!(out.contains("Upper-case") && out.contains("Reverse list"), "titles: {out}");
        assert!(out.contains("toUpper") && out.contains("reverseList"), "ids: {out}");
        // Multiple verbs for the first endpoint render as separate badges (2 + 1).
        assert_eq!(out.matches("cat-verb").count(), 3, "3 verb badges total: {out}");
    }
}

