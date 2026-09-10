# ikigai-xslt

An **XSLT transformation** module for the [ikigai-core](https://crates.io/crates/ikigai-core)
resolution kernel. It binds a single endpoint — `urn:xslt:transform` — that applies an
XSLT stylesheet to an XML source document and returns the styled result.

This is a standalone module crate: a host links it and mounts its endpoint into the
kernel's root with [`space()`](#mounting). Because the source and the stylesheet are
both resolved *through the kernel* as resource references, the transform composes with
the rest of the resource graph — and inherits its caching for free.

It is a general styling mechanism for arbitrary XML, RDF/XML in particular: the same
cached graph can be rendered into different presentations just by swapping the
stylesheet (e.g. turning a `urn:kernel:catalog` RDF/XML graph into a page of endpoint
cards).

## Arguments

`urn:xslt:transform?src=<…>&stylesheet=<uri>&as=<media-type>`

| Argument     | Required | Description |
| ------------ | -------- | ----------- |
| `stylesheet` | yes      | The XSLT stylesheet — a resolvable resource IRI (`urn:`, `file:`, or `http(s)://`), or the **stylesheet itself** (any value beginning with `<`). |
| `src`        | yes\*    | The source document. Either a resolvable resource IRI, **inline XML** (any value beginning with `<`), or the value piped in from a previous step. |
| `content`    | no       | The source document by value — where a pipeline's upstream value arrives. `src` takes precedence when both are given. |
| `as`         | no       | Output media type. Omitted, it follows the stylesheet's `xsl:output method` (see below). |

\* `src` may be omitted when the document is piped in — the engine routes a piped value
to the first input. An explicit `content=` argument is also accepted. So it slots into a
pipeline, e.g. `… | urn:rdf:transrept as=application/rdf+xml | urn:xslt:transform stylesheet=<uri>`.

Every input is declared with an XSD class (`xsd:string` for all four: `src` and
`stylesheet` are each a union of an IRI and a document, which no ArgSpec class states
more precisely), so the manifold and `urn:kernel:validate` can form and check a call.

## Output media type

The result is labeled by `as=` when given; otherwise by what the stylesheet's top-level
`xsl:output method` implies — the three declared outputs:

| `xsl:output method` | media type | serialization |
| ------------------- | ---------- | ------------- |
| `html` (or no `xsl:output`) | `text/html` | markup |
| `xml` | `application/xml` | markup |
| `text` | `text/plain` | the result's string value, whitespace preserved, nothing escaped |

A `method="text"` stylesheet is serialized as text whatever `as=` says, and `as=text/plain`
selects text serialization for any stylesheet. `as=` may relabel the markup with a type the
declaration does not list (`image/svg+xml` for an SVG-emitting stylesheet); the declared
three are what the endpoint chooses by itself.

`document()` is not supported: the transform runs synchronously and a kernel resolution
does not, so the engine's fetcher refuses every URL. A stylesheet that calls it fails with
a typed `Endpoint` error naming the function, and the referenced resource is never
resolved — there is no read for a capability to gate.

## Caching

Both `src` and `stylesheet`, when they are IRIs, are fetched with the kernel's own
resolution path — an `http(s)://` reference goes through the HTTP module
(`urn:httpGet`, gated by its `urn:cap:net:<host>` scope), any other IRI resolves
directly via `inv.source`. Either way the kernel records each referent's **golden
thread**, so the produced representation is `.cacheable()`: it is served from cache
until *either* the source or the stylesheet changes, at which point it auto-invalidates.
The transform therefore inherits the expiry and freshness of whatever it was built from —
a stylesheet served live makes the transform live too, and with both inputs inline it is
a pure function of them.

## Conformance

**Passes [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)** with
no opt-outs: `tests/conformance.rs` walks `urn:xslt:transform` and runs every check —
ArgSpecs, declared = enforced, the RDF faces (none: the output is the stylesheet's, not a
graph this module authors), the cacheable-twice probe, pipeline citizenship, naming. Three
walks: both inputs inline (declared `pure` and `cacheable`), both by reference under
threads (`cacheable` — the result carries `urn:file:foaf.xsl` and recomputes after a cut),
and both by reference served live (nothing is cached, and declaring otherwise is the one
red line). What the suite cannot see is pinned by hand in the same file: the declared
outputs against what each `xsl:output method` serves, `document()` never reaching the
kernel, and the `urn:cap:net` gate on a remote stylesheet.

## Pure Rust, wasm-ready

The transform is built on [`xrust`](https://crates.io/crates/xrust) (pure-Rust XPath 1.0
/ XSLT 1.0) — no C dependency, no `libxslt`, `#![forbid(unsafe_code)]`. It runs natively
and compiles to `wasm32` unchanged; the demo lazy-loads it in the browser as a WASM
module via the sibling `ikigai-xslt-module`. The public, host-agnostic
`transform_xml(src, stylesheet, text_output) -> Result<String, String>` entry point
carries no ikigai-core types so it can be wrapped directly.

## Usage

From the ikigai shell:

```shell
source urn:xslt:transform src=urn:data:catalog.rdf stylesheet=urn:style:cards as=text/html
```

## Mounting

```rust
use ikigai_core::{Fallback, Kernel, Space};
use std::sync::Arc;

// Mount `space()` (binds urn:xslt:transform) alongside the rest of your resources.
let root: Arc<dyn Space> = Arc::new(Fallback::new(vec![
    Arc::new(my_space) as Arc<dyn Space>,
    Arc::new(ikigai_xslt::space()) as Arc<dyn Space>,
]));
let kernel = Kernel::new(root);
```

## Run as a standalone module server

The same `space()` can run **out-of-process** behind a Unix socket, via
[`ikigai-module`](https://crates.io/crates/ikigai-module)'s `serve` — so a host
resolves `urn:xslt:transform` against it over a socket, and the module pulls its
`src`/`stylesheet` back through that same socket (the by-reference module session).
xrust then never has to be linked into the host. Two runnable examples show both ends:

```sh
# terminal 1 — the module server:
cargo run --example uds-server -- /tmp/ikigai-xslt.sock
# terminal 2 — a host that transforms a document through it:
cargo run --example uds-client -- /tmp/ikigai-xslt.sock
#   → transformed over the socket → "hello from across a socket"
```

(`ikigai-module` is a dev-dependency, used only by these examples — it isn't part of
the published library's dependency graph.)

## Build it as a browser WASM module

The same `space()` is *also* this library's own lazily-loadable WASM module — no separate
crate. The `module` feature pulls [`ikigai-module`](https://crates.io/crates/ikigai-module)
and emits the module glue via `ikigai_module::wasm_module!` (an `invoke_session` entry point
+ the host-callback bridge):

```sh
cargo build --release --lib --features module --target wasm32-unknown-unknown
# → target/wasm32-unknown-unknown/release/ikigai_xslt.wasm — wasm-bindgen it, lazy-load it,
#   resolve urn:xslt:transform against it; it pulls src/stylesheet back over the byte channel.
```

The feature is off by default and its deps (wasm-bindgen et al.) are optional, so a normal
(native/linked) consumer of the library never pulls them.

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`).
