//! `ikigai-xslt` — XSLT transformation as an ikigai resource.
//!
//! `urn:xslt:transform?src=<uri>&stylesheet=<uri>` applies an XSLT `stylesheet` to a
//! `src` document — **both resolved through the kernel as cacheable resource
//! references**. Because each is fetched with `inv.source`/`inv.issue`, the result
//! depends on both golden threads, so it is `.cacheable()` and auto-invalidates when
//! either the source or the stylesheet changes. `src` may instead be piped in (so it
//! also composes in a pipeline, e.g. `… | urn:rdf:transrept as=application/rdf+xml |
//! urn:xslt:transform stylesheet=<uri>`). Either may also be given **inline** — any value
//! beginning with `<` is the document itself, not a reference — and with both inline the
//! transform is a pure function of its inputs.
//!
//! The result's media type follows the stylesheet's `xsl:output method` (`html` →
//! `text/html`, `xml` → `application/xml`, `text` → `text/plain`) unless `as=` names one.
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
        // The stylesheet — a resolvable resource reference, or the stylesheet itself
        // inline (the same `<` discriminator as `src`: XML starts with `<`, an IRI never).
        let style_ref = inv.inline_str("stylesheet").map_err(|_| {
            Error::Endpoint(
                "urn:xslt:transform needs a `stylesheet=<uri>` resource reference".to_string(),
            )
        })?;
        let stylesheet = if is_inline_xml(style_ref) {
            style_ref.to_string()
        } else {
            utf8(resolve_ref(inv, style_ref).await?, "stylesheet")?
        };

        // The source document. `src` is either a resource IRI to resolve, or — when the
        // document is piped in (the engine routes a piped value to the first input) — the
        // inline XML itself. XML always starts with `<`, an IRI never does, so that's the
        // discriminator. An explicit `content=` is also accepted.
        let source = if let Ok(src) = inv.inline_str("src") {
            if is_inline_xml(src) {
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

        // The output media type: what `as=` names, else what the stylesheet's
        // `xsl:output method` implies (html is the default, as in XSLT itself — the
        // common case here is styling RDF/XML into a page).
        let method = stylesheet_output_method(&stylesheet).map_err(Error::Endpoint)?;
        let media = match inv.inline_str("as") {
            Ok(media) => media.to_string(),
            Err(_) => media_type_for(method.as_deref()).to_string(),
        };
        // A `method="text"` stylesheet — or a caller asking for `text/plain` — wants the
        // result's string value, whitespace preserved. Anything else is markup → XML
        // serialize.
        let text_output = method.as_deref() == Some("text")
            || media.split(';').next().unwrap_or(&media).trim() == "text/plain";

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
            // `src` and `stylesheet` are each EITHER a resource IRI OR the XML itself
            // (any value beginning with `<`). A union of `xsd:anyURI` and a document has
            // no ArgSpec spelling, so the class is the wire's type, `xsd:string`.
            .input(
                ArgSpec::new("src")
                    .summary(
                        "the source XML/RDF-XML document: a resolvable resource IRI, or the \
                         XML itself (or pipe it in)",
                    )
                    .class(XSD_STRING),
            )
            .input(
                ArgSpec::new("stylesheet")
                    .summary(
                        "the XSLT stylesheet: a resolvable resource IRI, or the stylesheet \
                         itself",
                    )
                    .class(XSD_STRING),
            )
            .input(
                ArgSpec::new("content")
                    .summary(
                        "the source document by value (a pipeline's upstream value); `src` \
                         takes precedence when both are given",
                    )
                    .class(XSD_STRING)
                    .optional(),
            )
            .input(
                ArgSpec::new("as")
                    .summary(
                        "output media type; omitted, it follows the stylesheet's xsl:output \
                         method (html → text/html, xml → application/xml, text → text/plain)",
                    )
                    .class(XSD_STRING)
                    .optional(),
            )
            // The three media types `xsl:output method` can imply. `as=` may relabel the
            // markup with any type (`image/svg+xml` for an SVG-emitting stylesheet); the
            // declared list is what the endpoint chooses by itself.
            .output("text/html;charset=utf-8")
            .output("application/xml;charset=utf-8")
            .output("text/plain;charset=utf-8")
            // A first-class `ik:Transreptor` for *discovery* — but a parameterized one:
            // it requires a `stylesheet`, so it is NOT auto-invocable (selection skips it,
            // since it can't be driven from `content` + `as` alone) and must be invoked
            // explicitly with its stylesheet. The matrix is indicative; the real output
            // is whatever the stylesheet emits.
            .transreptor(
                ["application/xml", "text/xml", "application/rdf+xml"],
                ["text/html", "application/xml", "text/plain"],
            )
    }
}

/// The XSD datatype every by-value input here declares: each is a string on the wire
/// (an IRI or a document), and no ArgSpec class states that union more precisely.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The XSLT namespace, as xrust's `QName` displays it (`{ns}local`).
const XSL_NS: &str = "http://www.w3.org/1999/XSL/Transform";

/// Whether an argument value is a document rather than a reference to one: XML always
/// starts with `<` (after leading whitespace) and an IRI never does.
fn is_inline_xml(value: &str) -> bool {
    value.trim_start().starts_with('<')
}

/// The media type an `xsl:output method` implies; `None` (no `xsl:output`) is XSLT's own
/// default, which — for an engine whose job is styling into pages — is `html`.
fn media_type_for(method: Option<&str>) -> &'static str {
    match method {
        Some("text") => "text/plain",
        Some("xml") => "application/xml",
        _ => "text/html",
    }
}

/// The `method` of a stylesheet's top-level `xsl:output` element (`"html"`, `"xml"`,
/// `"text"`, …), or `None` when the stylesheet declares none. Read before the transform
/// runs, so a host can label the result without inspecting it.
///
/// ```
/// let text = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
///   <xsl:output method="text"/>
///   <xsl:template match="/">x</xsl:template>
/// </xsl:stylesheet>"#;
/// assert_eq!(ikigai_xslt::stylesheet_output_method(text).unwrap().as_deref(), Some("text"));
/// let plain = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"/>"#;
/// assert_eq!(ikigai_xslt::stylesheet_output_method(plain).unwrap(), None);
/// ```
pub fn stylesheet_output_method(
    stylesheet_xml: &str,
) -> std::result::Result<Option<String>, String> {
    let doc =
        parse_xml(stylesheet_xml).map_err(|e| format!("stylesheet parse error: {}", e.message))?;
    let Some(root) = doc.child_iter().find(|c| c.is_element()) else {
        return Ok(None);
    };
    let output_name = format!("{{{XSL_NS}}}output");
    Ok(root
        .child_iter()
        .filter(|c| c.is_element())
        .filter(|c| c.name().is_some_and(|n| n.to_string() == output_name))
        .find_map(|output| {
            output
                .attribute_iter()
                .find(|a| a.name().is_some_and(|n| n.to_string() == "method"))
                .map(|a| a.value().to_string())
        }))
}

/// Resolve a resource reference through the kernel. An `http(s)://` URL is fetched via
/// the HTTP module (`urn:httpGet`); a `urn:`/`file:` IRI resolves directly. Either way
/// the kernel records the source's golden thread, so the transform is cacheable and
/// invalidates when the referenced resource changes.
async fn resolve_ref(inv: &Invocation<'_>, uri: &str) -> Result<Representation> {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        let get = Iri::parse("urn:httpGet").expect("urn:httpGet is a valid IRI");
        let request = Request::new(Verb::Source, get)
            .with_arg("url", ArgRef::Inline(uri.as_bytes().to_vec()));
        inv.issue(request).await
    } else {
        let iri = Iri::parse(uri)
            .map_err(|e| Error::Endpoint(format!("bad resource IRI `{uri}`: {e}")))?;
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
    xmlparse(
        doc.clone(),
        s,
        Some(|_: &_| Err(ParseError::MissingNameSpace)),
    )?;
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
        assert!(
            out.contains("Upper-case") && out.contains("Reverse list"),
            "titles: {out}"
        );
        assert!(
            out.contains("toUpper") && out.contains("reverseList"),
            "ids: {out}"
        );
        // Multiple verbs for the first endpoint render as separate badges (2 + 1).
        assert_eq!(
            out.matches("cat-verb").count(),
            3,
            "3 verb badges total: {out}"
        );
    }

    #[test]
    fn describes_itself_as_a_parameterized_transreptor() {
        let description = XsltEndpoint.describe();
        let t = description
            .transreption()
            .expect("xslt-transform is an ik:Transreptor");
        assert!(t.from.contains(&"application/rdf+xml".to_string()));
        assert!(t.to.contains(&"text/html".to_string()));
        // It needs a `stylesheet` input — that's what makes it a transreptor for
        // discovery but NOT auto-invocable: selection skips it because it can't be
        // driven from `content` + `as` alone.
        assert!(description.inputs.iter().any(|i| i.name == "stylesheet"));
    }
}

// ---------------------------------------------------------------------------
// This library *as* a dynamically-loadable WASM module.
//
// With `--features module` on `wasm32`, `wasm_module!` emits the cdylib glue — a public
// `invoke_session(Vec<u8>) -> Vec<u8>` that runs `ikigai_module::run_session` over our
// `space()`, plus the `hostCall` import + the !Send→Send bridge — so a host can lazy-load
// `ikigai_xslt.wasm` and resolve `urn:xslt:transform` against it (the module pulling its
// `src`/`stylesheet` back over the byte channel). The whole module is one macro line:
//
//   cargo build --release --lib --features module --target wasm32-unknown-unknown
// ---------------------------------------------------------------------------
#[cfg(feature = "module")]
ikigai_module::wasm_module!(crate::space);

/// Surface a Rust panic in the browser console (module builds only).
#[cfg(feature = "module")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn __module_start() {
    console_error_panic_hook::set_once();
}
