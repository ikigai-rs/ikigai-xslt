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
//!
//! A stylesheet may `xsl:import` / `xsl:include` others: each `href` is resolved
//! against the stylesheet's own IRI and fetched through the kernel before the engine
//! compiles, so a module is one more resource reference (and one more golden thread).

#![forbid(unsafe_code)]

use async_trait::async_trait;
use std::collections::HashMap;

use ikigai_core::{
    ArgRef, ArgSpec, Description, Endpoint, EndpointSpace, Error, Exact, Invocation, Iri, ReprType,
    Representation, Request, Result, Verb,
};
use url::Url;
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
        // Its `xsl:import`/`xsl:include` modules — static hrefs, each one more resource
        // reference resolved through the kernel (so one more thread the result depends on).
        let modules = resolve_modules(inv, style_uri, &stylesheet).await?;

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
        let out = transform_xml_with_modules(&source, &stylesheet, &modules, text_output)
            .map_err(Error::Endpoint)?;
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
            .input(ArgSpec::new("stylesheet").summary(
                "the XSLT stylesheet: a resolvable resource IRI; its xsl:import/xsl:include \
                 hrefs resolve against it and are fetched through the kernel too",
            ))
            .input(ArgSpec::new("as").summary("output media type (default text/html)"))
            .output("text/html;charset=utf-8")
            // A first-class `ik:Transreptor` for *discovery* — but a parameterized one:
            // it requires a `stylesheet`, so it is NOT auto-invocable (selection skips it,
            // since it can't be driven from `content` + `as` alone) and must be invoked
            // explicitly with its stylesheet. The matrix is indicative; the real output
            // is whatever the stylesheet emits.
            .transreptor(
                ["application/xml", "text/xml", "application/rdf+xml"],
                ["text/html", "text/plain"],
            )
    }
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

/// Fetch every `xsl:import` / `xsl:include` module a stylesheet names, through the
/// kernel, keyed by the href **as written** (which is how [`transform_xml_with_modules`]
/// wants them). A relative href resolves against the stylesheet's own IRI — the
/// `urn:file:` sibling is the common case — an absolute one is used as is. A module that
/// does not resolve is a typed error naming the href, never an empty document.
async fn resolve_modules(
    inv: &Invocation<'_>,
    style_uri: &str,
    stylesheet: &str,
) -> Result<HashMap<String, String>> {
    let mut modules = HashMap::new();
    for href in stylesheet_module_hrefs(stylesheet).map_err(Error::Endpoint)? {
        let iri = resolve_href(style_uri, &href)?;
        let repr = resolve_ref(inv, &iri)
            .await
            .map_err(|e| module_error(style_uri, &href, &iri, e))?;
        let text = utf8(repr, &format!("stylesheet module `{href}` ({iri})"))?;
        modules.insert(href, text);
    }
    Ok(modules)
}

/// Resolve a module `href` against the stylesheet's IRI: RFC 3986 reference resolution,
/// so `shared.xsl` and `../common/base.xsl` land beside the stylesheet whatever its
/// scheme (`urn:file:`, `file:`, `http(s):`), and an absolute href is returned unchanged.
fn resolve_href(style_uri: &str, href: &str) -> Result<String> {
    let base = oxiri::Iri::parse(style_uri).map_err(|e| {
        Error::Endpoint(format!(
            "stylesheet IRI `{style_uri}` cannot be the base for href `{href}`: {e}"
        ))
    })?;
    base.resolve(href).map(|iri| iri.into_inner()).map_err(|e| {
        Error::Endpoint(format!(
            "stylesheet `{style_uri}` has an unusable href `{href}`: {e}"
        ))
    })
}

/// The error for a module that did not come back: a not-found stays a typed
/// `NotFound`, any other failure a typed `Endpoint` error — both naming the href as
/// written, the IRI it resolved to, and the stylesheet that asked. A denial or a
/// transient failure keeps its own type (retry and capability overlays gate on it).
fn module_error(style_uri: &str, href: &str, iri: &str, e: Error) -> Error {
    let detail = format!(
        "stylesheet `{style_uri}` references module `{href}` (`{iri}`), which could not be fetched: {e}"
    );
    match e {
        Error::Denied(_) | Error::Timeout(_) | Error::Unavailable(_) => e,
        Error::Unresolved(_) | Error::NotFound(_) => Error::NotFound(detail),
        _ => Error::Endpoint(detail),
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
///
/// A stylesheet with `xsl:import` / `xsl:include` needs its modules supplied — see
/// [`transform_xml_with_modules`]; here, with none supplied, the first href fails
/// naming itself.
pub fn transform_xml(
    src_xml: &str,
    stylesheet_xml: &str,
    text_output: bool,
) -> std::result::Result<String, String> {
    transform_xml_with_modules(src_xml, stylesheet_xml, &HashMap::new(), text_output)
}

/// The base every module href is joined against before xrust sees it. xrust resolves
/// `xsl:import`/`xsl:include` hrefs with the `url` crate, which cannot join a relative
/// href onto a `urn:` base (not hierarchical) — so the *host* resolves each href to a
/// real IRI and fetches it, and the engine is handed a map keyed by this synthetic join.
/// The scheme never leaves the process: it exists only so the key xrust computes and the
/// key precomputed here are the same function of the same href.
const MODULE_BASE: &str = "x-ikigai-xslt://stylesheet/";

/// The XSLT namespace, as xrust's `QName` displays it (`{ns}local`).
const XSL_NS: &str = "http://www.w3.org/1999/XSL/Transform";

/// The `href`s of a stylesheet's top-level `xsl:include` and `xsl:import` elements, as
/// written, in document order. They are static attributes, so a host can resolve and
/// fetch every module *before* compiling — which is what [`transform_xml_with_modules`]
/// expects. An `xsl:include`/`xsl:import` without an `href` is an error. (xrust only
/// reads the top level of the main stylesheet: a module's own imports are silently
/// ignored, so only that level is collected.)
///
/// ```
/// let style = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
///   <xsl:import href="shared.xsl"/>
///   <xsl:include href="urn:style:labels.xsl"/>
///   <xsl:template match="/">x</xsl:template>
/// </xsl:stylesheet>"#;
/// let hrefs = ikigai_xslt::stylesheet_module_hrefs(style).unwrap();
/// assert_eq!(hrefs, ["shared.xsl", "urn:style:labels.xsl"]);
/// ```
pub fn stylesheet_module_hrefs(stylesheet_xml: &str) -> std::result::Result<Vec<String>, String> {
    let doc =
        parse_xml(stylesheet_xml).map_err(|e| format!("stylesheet parse error: {}", e.message))?;
    let Some(root) = doc.child_iter().find(|c| c.is_element()) else {
        return Ok(Vec::new());
    };
    let mut hrefs = Vec::new();
    for child in root.child_iter().filter(|c| c.is_element()) {
        let name = child.name().map(|n| n.to_string()).unwrap_or_default();
        let Some(local) = name.strip_prefix(&format!("{{{XSL_NS}}}")) else {
            continue;
        };
        if local != "include" && local != "import" {
            continue;
        }
        let href = child
            .attribute_iter()
            .find(|a| a.name().is_some_and(|n| n.to_string() == "href"))
            .map(|a| a.value().to_string())
            .filter(|h| !h.is_empty());
        match href {
            Some(h) => hrefs.push(h),
            None => return Err(format!("xsl:{local} without an href")),
        }
    }
    Ok(hrefs)
}

/// [`transform_xml`] for a stylesheet that imports or includes others. `modules` maps
/// each href **as written** in the stylesheet (see [`stylesheet_module_hrefs`]) to that
/// module's XML text — the host has already resolved and fetched them, so this stays
/// synchronous and I/O-free (wasm-clean). A module the stylesheet names but the map
/// lacks is an error naming the href; nothing is ever substituted for it.
///
/// ```
/// use std::collections::HashMap;
/// let main = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
///   <xsl:import href="shared.xsl"/>
///   <xsl:output method="text"/>
///   <xsl:template match="/"><xsl:apply-templates select="doc/item"/></xsl:template>
/// </xsl:stylesheet>"#;
/// let shared = r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
///   <xsl:template match="item">[<xsl:value-of select="."/>]</xsl:template>
/// </xsl:stylesheet>"#;
/// let modules = HashMap::from([("shared.xsl".to_string(), shared.to_string())]);
/// let out = ikigai_xslt::transform_xml_with_modules(
///     "<doc><item>a</item><item>b</item></doc>", main, &modules, true).unwrap();
/// assert_eq!(out, "[a][b]");
/// ```
pub fn transform_xml_with_modules(
    src_xml: &str,
    stylesheet_xml: &str,
    modules: &HashMap<String, String>,
    text_output: bool,
) -> std::result::Result<String, String> {
    let srcdoc =
        parse_xml(src_xml).map_err(|e| format!("source document parse error: {}", e.message))?;
    let styledoc =
        parse_xml(stylesheet_xml).map_err(|e| format!("stylesheet parse error: {}", e.message))?;

    // Key the modules the way xrust will ask for them: `MODULE_BASE.join(href)`.
    let base = Url::parse(MODULE_BASE).expect("MODULE_BASE is a valid URL");
    let mut keyed: HashMap<String, &String> = HashMap::with_capacity(modules.len());
    for (href, text) in modules {
        let key = base.join(href).map_err(|e| {
            format!("stylesheet module href `{href}` is not a valid reference: {e}")
        })?;
        keyed.insert(key.into(), text);
    }
    let fetch_module = |url: &Url| {
        keyed
            .get(url.as_str())
            .map(|t| t.to_string())
            .ok_or_else(|| {
                let href = url
                    .as_str()
                    .strip_prefix(MODULE_BASE)
                    .unwrap_or(url.as_str());
                XsltError::new(
                    XsltErrorKind::Unknown,
                    format!("stylesheet module `{href}` was not supplied"),
                )
            })
    };

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

    let mut ctxt = from_document(styledoc, Some(base), parse_xml, fetch_module)
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

/// Test helper: the public [`transform_xml`] mapped into an ikigai error.
#[cfg(test)]
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

    const XSL: &str = "http://www.w3.org/1999/XSL/Transform";

    #[test]
    fn collects_top_level_import_and_include_hrefs_in_order() {
        let style = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}">
              <xsl:import href="../common/base.xsl"/>
              <xsl:include href="urn:style:labels.xsl"/>
              <xsl:template match="/"><xsl:include href="not-top-level.xsl"/></xsl:template>
            </xsl:stylesheet>"#
        );
        assert_eq!(
            stylesheet_module_hrefs(&style).unwrap(),
            ["../common/base.xsl", "urn:style:labels.xsl"]
        );
        // No modules at all is an empty list, not an error.
        let plain = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:template match="/"/></xsl:stylesheet>"#
        );
        assert!(stylesheet_module_hrefs(&plain).unwrap().is_empty());
        // A module element without an href is refused.
        let bare = format!(r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:import/></xsl:stylesheet>"#);
        let err = stylesheet_module_hrefs(&bare).unwrap_err();
        assert!(err.contains("xsl:import without an href"), "{err}");
    }

    #[test]
    fn imports_and_includes_the_supplied_modules() {
        let main = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}">
              <xsl:import href="shared.xsl"/>
              <xsl:include href="urn:style:labels.xsl"/>
              <xsl:output method="text"/>
              <xsl:template match="/"><xsl:value-of select="$label"/>: <xsl:apply-templates select="doc/item"/></xsl:template>
            </xsl:stylesheet>"#
        );
        // The imported module supplies the `item` template …
        let shared = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:template match="item">[<xsl:value-of select="."/>]</xsl:template></xsl:stylesheet>"#
        );
        // … and the included one a top-level variable.
        let labels = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:variable name="label" select="'items'"/></xsl:stylesheet>"#
        );
        let modules = HashMap::from([
            ("shared.xsl".to_string(), shared),
            ("urn:style:labels.xsl".to_string(), labels),
        ]);
        let out = transform_xml_with_modules(
            "<doc><item>a</item><item>b</item></doc>",
            &main,
            &modules,
            true,
        )
        .expect("transform with modules");
        assert_eq!(out, "items: [a][b]");
    }

    #[test]
    fn a_module_that_was_not_supplied_is_named_never_substituted() {
        let main = format!(
            r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:import href="shared.xsl"/><xsl:template match="/">x</xsl:template></xsl:stylesheet>"#
        );
        let err = transform_xml("<a/>", &main, true).unwrap_err();
        assert!(err.contains("`shared.xsl` was not supplied"), "{err}");
    }

    #[test]
    fn resolves_module_hrefs_against_the_stylesheet_iri() {
        let base = "urn:file:/Users/b/site/style/main.xsl";
        assert_eq!(
            resolve_href(base, "shared.xsl").unwrap(),
            "urn:file:/Users/b/site/style/shared.xsl"
        );
        assert_eq!(
            resolve_href(base, "../common/base.xsl").unwrap(),
            "urn:file:/Users/b/site/common/base.xsl"
        );
        assert_eq!(
            resolve_href(base, "urn:style:labels.xsl").unwrap(),
            "urn:style:labels.xsl"
        );
        assert_eq!(
            resolve_href("https://example.org/xsl/main.xsl", "inc/a.xsl").unwrap(),
            "https://example.org/xsl/inc/a.xsl"
        );
        assert_eq!(
            resolve_href("urn:test:style/main.xsl", "shared.xsl").unwrap(),
            "urn:test:style/shared.xsl"
        );
    }

    /// A kernel with a test space: the stylesheet imports a sibling by relative href,
    /// and the endpoint resolves that sibling through the kernel.
    mod in_kernel {
        use super::*;
        use futures::executor::block_on;
        use ikigai_core::{Capability, Fallback, FnEndpoint, Kernel, Space};
        use std::sync::Arc;

        fn fixed(media: &'static str, body: String) -> FnEndpoint {
            FnEndpoint::new("fixed", move |_inv| {
                Ok(
                    Representation::new(ReprType::new(media), body.clone().into_bytes())
                        .cacheable(),
                )
            })
        }

        fn kernel(main_xsl: String) -> Kernel {
            let shared = format!(
                r#"<xsl:stylesheet xmlns:xsl="{XSL}"><xsl:template match="item">[<xsl:value-of select="."/>]</xsl:template></xsl:stylesheet>"#
            );
            let test = EndpointSpace::new()
                .bind(
                    Exact::new("urn:test:src"),
                    fixed(
                        "application/xml",
                        "<doc><item>a</item><item>b</item></doc>".into(),
                    ),
                )
                .bind(
                    Exact::new("urn:test:style/main.xsl"),
                    fixed("text/xml", main_xsl),
                )
                .bind(
                    Exact::new("urn:test:style/shared.xsl"),
                    fixed("text/xml", shared),
                );
            let root: Arc<dyn Space> = Arc::new(Fallback::new(vec![
                Arc::new(test) as Arc<dyn Space>,
                Arc::new(space()) as Arc<dyn Space>,
            ]));
            Kernel::new(root)
        }

        fn request() -> Request {
            Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
                .with_arg("src", ArgRef::Inline(b"urn:test:src".to_vec()))
                .with_arg(
                    "stylesheet",
                    ArgRef::Inline(b"urn:test:style/main.xsl".to_vec()),
                )
                .with_arg("as", ArgRef::Inline(b"text/plain".to_vec()))
        }

        #[test]
        fn resolves_an_imported_sibling_through_the_kernel() {
            let main = format!(
                r#"<xsl:stylesheet xmlns:xsl="{XSL}">
                  <xsl:import href="shared.xsl"/>
                  <xsl:output method="text"/>
                  <xsl:template match="/"><xsl:apply-templates select="doc/item"/></xsl:template>
                </xsl:stylesheet>"#
            );
            let rep = block_on(kernel(main).issue(request(), &Capability::root()))
                .expect("transform through the kernel");
            assert_eq!(String::from_utf8(rep.bytes).unwrap(), "[a][b]");
        }

        #[test]
        fn an_import_that_does_not_resolve_is_a_not_found_naming_it() {
            let main = format!(
                r#"<xsl:stylesheet xmlns:xsl="{XSL}">
                  <xsl:import href="missing.xsl"/>
                  <xsl:template match="/">x</xsl:template>
                </xsl:stylesheet>"#
            );
            let err = block_on(kernel(main).issue(request(), &Capability::root()))
                .expect_err("a missing module must fail");
            let Error::NotFound(msg) = &err else {
                panic!("expected NotFound, got {err:?}");
            };
            assert!(
                msg.contains("`missing.xsl`"),
                "names the href as written: {msg}"
            );
            assert!(
                msg.contains("`urn:test:style/missing.xsl`"),
                "names the IRI it resolved to: {msg}"
            );
            assert!(
                msg.contains("urn:test:style/main.xsl"),
                "names the stylesheet: {msg}"
            );
        }
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
