//! The module recipe as one test: `ikigai-conformance` walks the one endpoint
//! [`ikigai_xslt::space`] binds and reports every violation at once.
//!
//! ## A transformation with two by-reference inputs
//!
//! `urn:xslt:transform` takes a source document and a stylesheet, each EITHER a
//! resource IRI resolved through the kernel OR the XML itself (a value beginning
//! with `<`). With both inline the result is a pure function of its inputs, and
//! [`conforms`] declares it `pure` and `cacheable` over an inline fixture. A
//! reference is a sub-resolution whose expiry and golden threads the kernel folds
//! into the result: over a stylesheet served under a thread (what `ikigai-fs` does
//! for `urn:file:foaf.xsl`) the transform is cached under that thread and recomputes
//! after a cut ([`a_stylesheet_by_reference_inherits_its_thread`]); over a
//! stylesheet served live it is not cached at all
//! ([`over_a_live_stylesheet_nothing_is_cached`]). So the declarations certify
//! behavior over the kernel passed, not the module in isolation.
//!
//! ## No RDF face
//!
//! The output is whatever the stylesheet emits — markup or text, never a graph this
//! module authors — so the RDF checks have nothing to read and pass on every walk.
//! The `application/rdf+xml` INPUT is the caller's graph and is not examined.
//!
//! ## What the suite cannot see, pinned by hand
//!
//! - **Declared outputs against what is served, both directions**
//!   ([`declared_outputs_are_the_media_types_served`]): the suite compares the two
//!   only for RDF faces. The three declared outputs are the three `xsl:output
//!   method`s; `as=` may relabel the markup with any type, and that exception is
//!   pinned as the exception.
//! - **`document()`** ([`document_is_refused_before_the_kernel_is_asked`]): xrust
//!   asks a fetcher for it and this module's fetcher refuses — the kernel is never
//!   consulted, so no capability gates it and no `Denied` can arise. The pin is that
//!   a stylesheet calling `document()` fails with a typed `Endpoint` error naming
//!   the function and the referenced resource is never resolved, under root or
//!   under no grants alike.
//! - **A remote stylesheet** ([`a_remote_stylesheet_is_gated_by_the_net_capability`]):
//!   an `http(s)://` reference is fetched through `urn:httpGet`, a resource another
//!   module binds and gates with `urn:cap:net:<host>`. The transform declares no
//!   capability of its own, correctly — with `urn:`/inline inputs it reaches no
//!   network. The gate lives in the sub-resolution, which ENFORCED cannot see with
//!   the fixture's inline inputs, so the typed `Denied` is pinned here.
//!
//! No opt-outs, no module namespace, and NAMES runs: the id is kebab-case.

use ikigai_conformance::{Check, Fixture, Report, Suite};
use ikigai_core::{
    ArgRef, Capability, Description, Error, Exact, FnEndpoint, Invocation, Iri, Kernel, ReprType,
    Representation, Request, Verb,
};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// The one endpoint `space()` binds, by description id and by IRI.
const TRANSFORM: &str = "xslt-transform";
const TRANSFORM_IRI: &str = "urn:xslt:transform";

const XSL: &str = "http://www.w3.org/1999/XSL/Transform";

/// The walk's document: one FOAF person, in RDF/XML — the input the module exists for.
const DOC: &str = r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
  xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <foaf:Person rdf:about="http://example.org/ada"><foaf:name>Ada</foaf:name></foaf:Person>
</rdf:RDF>"#;

/// A page stylesheet with no `xsl:output`: XSLT's default method, html.
const STYLE_HTML: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:template match="/"><h1 class="name"><xsl:value-of select="//foaf:name"/></h1></xsl:template>
</xsl:stylesheet>"#;

/// The same page with a different class, so a recomputation after a cut is visible
/// in the bytes.
const STYLE_LABEL: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:template match="/"><h1 class="label"><xsl:value-of select="//foaf:name"/></h1></xsl:template>
</xsl:stylesheet>"#;

/// `method="xml"`.
const STYLE_XML: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:output method="xml"/>
  <xsl:template match="/"><name><xsl:value-of select="//foaf:name"/></name></xsl:template>
</xsl:stylesheet>"#;

/// `method="text"`, emitting characters that XML serialization would escape.
const STYLE_TEXT: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:output method="text"/>
  <xsl:template match="/">&lt;<xsl:value-of select="//foaf:name"/>&gt;</xsl:template>
</xsl:stylesheet>"#;

/// Where the by-reference walks bind the stylesheet and the document — the edge's
/// own names for them — doubling as the golden threads the threaded variant names
/// (the `ikigai-fs` convention: `depends_on` the resource's own IRI).
const STYLESHEET_IRI: &str = "urn:file:foaf.xsl";
const DOCUMENT_IRI: &str = "urn:file:brian.rdf";

/// The remote stylesheet the stand-in `urn:httpGet` serves, and the scope it demands.
const REMOTE_STYLESHEET: &str = "https://example.org/foaf.xsl";
const NET_SCOPE: &str = "urn:cap:net:example.org";

/// The suite with one fixture for the one action: `src` and `stylesheet` as the
/// kernel under test wants them — inline XML, or IRIs it binds.
fn suite(src: &str, stylesheet: &str) -> Suite {
    Suite::new().fixture(
        Fixture::new(TRANSFORM, Verb::Source)
            .arg("src", src)
            .arg("stylesheet", stylesheet),
    )
}

/// A file resource — what `urn:file:<name>` is to the module: bytes behind an IRI,
/// resolved through the kernel. `threaded` is a store that names its own IRI as the
/// golden thread (and cuts it on a write); `false` is a live store, served
/// uncacheable and read every time. The suite walks this endpoint beside the
/// module's, so it describes itself the way a module endpoint must.
fn file_resource(
    id: &'static str,
    media: &'static str,
    store: Arc<RwLock<&'static str>>,
    threaded: bool,
) -> FnEndpoint {
    FnEndpoint::new(id, move |inv: &Invocation<'_>| {
        let bytes = store.read().expect("store lock").as_bytes().to_vec();
        let repr = Representation::new(ReprType::new(media), bytes);
        Ok(if threaded {
            repr.cacheable().depends_on(inv.request.target.as_str())
        } else {
            repr
        })
    })
    .with_description(
        Description::new(id)
            .title("Conformance file resource")
            .summary("A workspace file served as a kernel resource for the walk.")
            .verb(Verb::Source)
            .output(media),
    )
}

/// The module's space plus the stylesheet at [`STYLESHEET_IRI`] and the document at
/// [`DOCUMENT_IRI`], and a handle to change the stylesheet in place. `threaded`
/// selects the store's kind (see the file docs).
fn with_files(threaded: bool) -> (Kernel, Arc<RwLock<&'static str>>) {
    let stylesheet = Arc::new(RwLock::new(STYLE_HTML));
    let document = Arc::new(RwLock::new(DOC));
    let space = ikigai_xslt::space()
        .bind(
            Exact::new(STYLESHEET_IRI),
            file_resource(
                "foaf-xsl",
                "application/xml",
                Arc::clone(&stylesheet),
                threaded,
            ),
        )
        .bind(
            Exact::new(DOCUMENT_IRI),
            file_resource("brian-rdf", "application/rdf+xml", document, threaded),
        );
    (Kernel::new(Arc::new(space)), stylesheet)
}

fn request(args: &[(&str, &str)]) -> Request {
    let mut request = Request::new(Verb::Source, Iri::parse(TRANSFORM_IRI).unwrap());
    for &(name, value) in args {
        request = request.with_arg(name, ArgRef::Inline(value.as_bytes().to_vec()));
    }
    request
}

fn by_reference() -> Request {
    request(&[("src", DOCUMENT_IRI), ("stylesheet", STYLESHEET_IRI)])
}

fn issue(kernel: &Kernel, request: Request, capability: &Capability) -> Result<String, Error> {
    futures::executor::block_on(kernel.issue(request, capability))
        .map(|repr| String::from_utf8(repr.bytes).expect("the output is UTF-8"))
}

/// The walk saw the module's endpoint plus `extra` fixture endpoints, one Source
/// action each (Meta is not an action), and skipped nothing. A second module
/// endpoint bound without a line here would be held to a weaker standard.
fn assert_shape(report: &Report, extra: usize) {
    assert_eq!(report.endpoints, 1 + extra, "{report}");
    assert_eq!(
        report.actions,
        1 + extra,
        "one Source action each: {report}"
    );
    assert_eq!(
        report.checks.skipped().count(),
        0,
        "every check runs: {report}"
    );
}

#[test]
fn conforms() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let report = suite(DOC, STYLE_HTML)
        .pure(TRANSFORM)
        .cacheable(TRANSFORM)
        .run_blocking(&kernel);
    // Printed even when clean (`--nocapture`): the report is the record.
    eprintln!("{report}");
    assert!(report.is_clean(), "{report}");
    assert_shape(&report, 0);
}

/// Both inputs by IRI, served under threads: the transform is cached, carries BOTH
/// threads — `urn:file:foaf.xsl` among them, which is what lets the edge's FOAF page
/// recompute when the stylesheet on disk is edited — and survives a change to the
/// stylesheet until that thread is cut. The cut is the store's (or a watcher's) job,
/// not this module's: it holds nothing and watches nothing. The suite walk over this
/// kernel is clean with the transform declared `cacheable` but NOT `pure`: its thread
/// set is its inputs', non-empty.
#[test]
fn a_stylesheet_by_reference_inherits_its_thread() {
    let (kernel, stylesheet) = with_files(true);

    let report = suite(DOCUMENT_IRI, STYLESHEET_IRI)
        .cacheable(TRANSFORM)
        .run_blocking(&kernel);
    eprintln!("[threaded files]\n{report}");
    assert!(report.is_clean(), "{report}");
    assert_shape(&report, 2);

    let first = issue(&kernel, by_reference(), &Capability::root()).unwrap();
    assert!(
        first.contains("class='name'") || first.contains("class=\"name\""),
        "{first}"
    );
    assert!(
        kernel.is_cached(&by_reference(), &Capability::root()),
        "over threaded inputs the transform is cached"
    );
    let repr =
        futures::executor::block_on(kernel.issue(by_reference(), &Capability::root())).unwrap();
    let threads: BTreeSet<String> = repr.threads().iter().map(|t| t.to_string()).collect();
    for thread in [STYLESHEET_IRI, DOCUMENT_IRI] {
        assert!(
            threads.contains(thread),
            "the transform carries `{thread}`: {threads:?}"
        );
    }

    // The stylesheet changes in its store. Nothing in this module notices.
    *stylesheet.write().expect("store lock") = STYLE_LABEL;
    let stale = issue(&kernel, by_reference(), &Capability::root()).unwrap();
    assert_eq!(
        stale, first,
        "no watcher here: a change with no cut is served from the cache"
    );

    // The store cuts the thread it named, and the transform goes with it.
    kernel.cut(STYLESHEET_IRI);
    let fresh = issue(&kernel, by_reference(), &Capability::root()).unwrap();
    assert!(
        fresh.contains("label"),
        "recomputed against the edited stylesheet after the cut: {fresh}"
    );
    assert_ne!(fresh, first);
}

/// The other store: the files served uncacheable. The module still says
/// `.cacheable()`, and the kernel hands the transform back uncacheable — the
/// effective expiry is the least cacheable input's. Undeclared, that is correct and
/// the walk is clean; DECLARED `cacheable`, the suite reports the downgrade on the
/// transform alone, which is the only way a silent recompute-every-read becomes
/// visible — the types are identical either way.
#[test]
fn over_a_live_stylesheet_nothing_is_cached() {
    let (kernel, _) = with_files(false);

    let report = suite(DOCUMENT_IRI, STYLESHEET_IRI).run_blocking(&kernel);
    eprintln!("[live files, undeclared]\n{report}");
    assert!(report.is_clean(), "{report}");
    assert_shape(&report, 2);
    issue(&kernel, by_reference(), &Capability::root()).unwrap();
    assert!(
        !kernel.is_cached(&by_reference(), &Capability::root()),
        "a transform over an uncacheable input is not cached"
    );

    let report = suite(DOCUMENT_IRI, STYLESHEET_IRI)
        .cacheable(TRANSFORM)
        .run_blocking(&kernel);
    eprintln!("[live files, transform declared cacheable]\n{report}");
    let downgraded: Vec<&str> = report
        .of(Check::Cacheable)
        .map(|f| f.endpoint.as_str())
        .collect();
    assert_eq!(downgraded, [TRANSFORM], "{report}");
    assert!(
        report.findings[0].detail.contains("declared cacheable"),
        "the finding names the declaration: {}",
        report.findings[0]
    );
    assert_eq!(report.findings.len(), 1, "and nothing else: {report}");
}

/// What `ikigai-conformance` 0.1.0 does not check (its PENDING #11): a declared
/// output is compared with what the action serves only when it is an RDF face, and
/// only in one direction. Read by hand, then pinned both ways: with `as` omitted the
/// media type follows the stylesheet's `xsl:output method`, the three methods serve
/// exactly the three declared outputs, and `method="text"` is the string value
/// (unescaped) even though nobody asked for `text/plain`. Then the one documented
/// exception: `as=` relabels the markup with a type the declaration does not list.
#[test]
fn declared_outputs_are_the_media_types_served() {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let declared: BTreeSet<String> = kernel
        .describe_pattern(TRANSFORM_IRI)
        .expect("the transform describes itself")
        .outputs
        .iter()
        .map(|o| ikigai_conformance::rdf::bare_media_type(o))
        .collect();

    let mut served = BTreeSet::new();
    for (stylesheet, expected_type, expected_body) in [
        (STYLE_HTML, "text/html", "Ada</h1>"),
        (STYLE_XML, "application/xml", "<name>Ada</name>"),
        (STYLE_TEXT, "text/plain", "<Ada>"),
    ] {
        let repr = futures::executor::block_on(kernel.issue(
            request(&[("src", DOC), ("stylesheet", stylesheet)]),
            &Capability::root(),
        ))
        .unwrap_or_else(|e| panic!("{expected_type}: {e}"));
        let got = ikigai_conformance::rdf::bare_media_type(&repr.repr_type.media_type);
        assert_eq!(got, expected_type, "the media type follows xsl:output");
        assert!(declared.contains(&got), "`{got}` is declared: {declared:?}");
        let body = String::from_utf8(repr.bytes).unwrap();
        assert!(body.contains(expected_body), "{expected_type}: {body}");
        served.insert(got);
    }
    assert_eq!(
        served, declared,
        "every declared output is served by some method"
    );

    // The exception: the caller's label wins, and it need not be declared.
    let repr = futures::executor::block_on(kernel.issue(
        request(&[
            ("src", DOC),
            ("stylesheet", STYLE_XML),
            ("as", "image/svg+xml"),
        ]),
        &Capability::root(),
    ))
    .unwrap();
    let got = ikigai_conformance::rdf::bare_media_type(&repr.repr_type.media_type);
    assert_eq!(got, "image/svg+xml");
    assert!(
        !declared.contains(&got),
        "a relabel is outside the declaration"
    );
}

/// `document()` never reaches the kernel. xrust hands the call to a fetcher, and this
/// module's fetcher refuses every URL (the transform is synchronous; a kernel
/// resolution is not) — so the referenced resource is not resolved, its capability
/// is not consulted, and the failure is a typed `Endpoint` error naming the function,
/// under root and under no grants alike. That is the honest pin: there is no cap gate
/// on `document()` to enforce because there is no read.
#[test]
fn document_is_refused_before_the_kernel_is_asked() {
    let touched = Arc::new(AtomicBool::new(false));
    let secret = {
        let touched = Arc::clone(&touched);
        FnEndpoint::new("secret", move |_inv: &Invocation<'_>| {
            touched.store(true, Ordering::SeqCst);
            Ok(Representation::new(
                ReprType::new("application/xml"),
                b"<secret/>".to_vec(),
            ))
        })
        .with_description(
            Description::new("secret")
                .title("A gated document")
                .summary("What document() would read, if it read through the kernel.")
                .verb(Verb::Source)
                .requires("urn:cap:secret:*")
                .output("application/xml"),
        )
    };
    let space = ikigai_xslt::space().bind(Exact::new("urn:secret:x"), secret);
    let kernel = Kernel::new(Arc::new(space));
    let stylesheet = format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="{XSL}">
  <xsl:template match="/"><xsl:copy-of select="document('urn:secret:x')"/></xsl:template>
</xsl:stylesheet>"#
    );

    for capability in [Capability::root(), Capability::scoped(Vec::<String>::new())] {
        match issue(
            &kernel,
            request(&[("src", DOC), ("stylesheet", &stylesheet)]),
            &capability,
        ) {
            Err(Error::Endpoint(msg)) => {
                assert!(msg.contains("document()"), "names the function: {msg}")
            }
            other => panic!("expected a typed Endpoint error, got {other:?}"),
        }
    }
    assert!(
        !touched.load(Ordering::SeqCst),
        "document() reached the kernel: the pin in this test is stale"
    );
}

/// The remote-stylesheet path, over a stand-in for `ikigai-http`'s `urn:httpGet`
/// that declares (and so, by the kernel's floor, enforces) a `urn:cap:net:` scope.
/// Under no grants the fetch is refused and the transform propagates the typed
/// `Denied`; under the host scope it succeeds. Over the module's own space, with no
/// `urn:httpGet` bound, the same call is `Unresolved`: reachability of a remote
/// stylesheet is the HOST's to provide, and the CALLER's to be granted.
#[test]
fn a_remote_stylesheet_is_gated_by_the_net_capability() {
    let http_get = FnEndpoint::new("http-get", |inv: &Invocation<'_>| {
        assert_eq!(inv.inline_str("url")?, REMOTE_STYLESHEET);
        Ok(Representation::new(
            ReprType::new("application/xml"),
            STYLE_HTML.as_bytes().to_vec(),
        ))
    })
    .with_description(
        Description::new("http-get")
            .title("Stand-in urn:httpGet")
            .summary("Serves one remote stylesheet, gated the way ikigai-http gates a host.")
            .verb(Verb::Source)
            .requires(NET_SCOPE)
            .output("application/xml"),
    );
    let space = ikigai_xslt::space().bind(Exact::new("urn:httpGet"), http_get);
    let kernel = Kernel::new(Arc::new(space));
    let remote = || request(&[("src", DOC), ("stylesheet", REMOTE_STYLESHEET)]);

    let none = Capability::scoped(Vec::<String>::new());
    match issue(&kernel, remote(), &none) {
        Err(Error::Denied(_)) => {}
        other => panic!("expected a typed Denied under no grants, got {other:?}"),
    }

    let granted = Capability::scoped([NET_SCOPE]);
    let body = issue(&kernel, remote(), &granted).unwrap();
    assert!(body.contains("Ada"), "{body}");

    let bare = Kernel::new(Arc::new(ikigai_xslt::space()));
    match issue(&bare, remote(), &Capability::root()) {
        Err(Error::Unresolved(iri)) => assert_eq!(iri.as_str(), "urn:httpGet"),
        other => panic!("expected Unresolved with no urn:httpGet bound, got {other:?}"),
    }
}
