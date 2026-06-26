//! A host that transforms a document through an out-of-process XSLT module — see
//! `uds-server.rs`. The module (the server) owns no resources of its own; it resolves the
//! `src` and `stylesheet` IRIs **back through this host's kernel** over the socket (the
//! by-reference session), then runs the transform and returns the result.
//!
//! Run `uds-server` first, then: `cargo run --example uds-client -- /tmp/ikigai-xslt.sock`

use std::path::PathBuf;
use std::sync::Arc;

use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, EndpointSpace, Exact, Fallback, FnEndpoint, Iri, Kernel, ReprType,
    Representation, Request, Space, Verb,
};
use ikigai_module::{ModuleSpace, UdsTransport};

// The two resources the module will reach back for. They live ONLY on this host — the
// module server has never heard of them; it learns them through the callback channel.
const SRC: &str = "<doc><msg>hello from across a socket</msg></doc>";
const STYLE: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="/doc/msg"/></xsl:template>
</xsl:stylesheet>"#;

fn fixed(media: &'static str, body: &'static str) -> FnEndpoint {
    FnEndpoint::new("demo", move |_inv| {
        Ok(Representation::new(ReprType::new(media), body.as_bytes().to_vec()).cacheable())
    })
}

fn socket_path() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ikigai-xslt.sock"))
}

fn main() {
    let path = socket_path();

    // Host kernel: the demo resources locally, and `urn:xslt:*` routed to the module
    // server over the Unix socket.
    let host = EndpointSpace::new()
        .bind(Exact::new("urn:demo:src"), fixed("application/xml", SRC))
        .bind(Exact::new("urn:demo:style"), fixed("text/xml", STYLE));
    let module = ModuleSpace::new(["urn:xslt:"], Arc::new(UdsTransport::connect(&path)));
    let root: Arc<dyn Space> = Arc::new(Fallback::new(vec![
        Arc::new(host) as Arc<dyn Space>,
        Arc::new(module) as Arc<dyn Space>,
    ]));
    let kernel = Kernel::new(root);

    let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"urn:demo:src".to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(b"urn:demo:style".to_vec()))
        .with_arg("as", ArgRef::Inline(b"text/plain".to_vec()));

    match block_on(kernel.issue(request, &Capability::root())) {
        Ok(rep) => println!(
            "transformed over the socket → {:?}",
            String::from_utf8_lossy(&rep.bytes)
        ),
        Err(e) => {
            eprintln!(
                "error: {e}  (is the uds-server running on {}?)",
                path.display()
            );
            std::process::exit(1);
        }
    }
}
