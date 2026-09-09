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
| `stylesheet` | yes      | The XSLT stylesheet — a resolvable resource IRI (`urn:`, `file:`, or `http(s)://`). |
| `src`        | yes\*    | The source document. Either a resolvable resource IRI, **inline XML** (any value beginning with `<`), or the value piped in from a previous step. |
| `as`         | no       | Output media type. Default `text/html`. `text/plain` serializes the result's string value (a `method="text"` stylesheet); anything else is serialized as XML/markup. |

\* `src` may be omitted when the document is piped in — the engine routes a piped value
to the first input. An explicit `content=` argument is also accepted. So it slots into a
pipeline, e.g. `… | urn:rdf:transrept as=application/rdf+xml | urn:xslt:transform stylesheet=<uri>`.

## Importing and including stylesheets

A stylesheet may `xsl:import` / `xsl:include` others. The engine (xrust) does not fetch
modules itself — it asks a fetcher for each `href`, and before this module resolved them
every href came back as an empty document, so any importing stylesheet failed with
"expected `<`". Now the hrefs — static attributes, so they are known before compiling —
are collected from the stylesheet, resolved **against the stylesheet's own IRI** (RFC 3986
reference resolution), and fetched **through the kernel** exactly like the stylesheet
itself: a module is one more resource reference, and one more golden thread the cached
result depends on.

```xml
<!-- urn:file:/site/style/main.xsl -->
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:import href="shared.xsl"/>            <!-- → urn:file:/site/style/shared.xsl -->
  <xsl:include href="../common/labels.xsl"/> <!-- → urn:file:/site/common/labels.xsl -->
  <xsl:include href="urn:style:footer"/>     <!-- absolute: used as is -->
  …
</xsl:stylesheet>
```

A module that does not resolve is a **typed error naming the href**, the IRI it resolved
to, and the stylesheet that asked (`NotFound` for an absent resource, `Endpoint` for any
other failure; a denial or a transient failure keeps its own type). Nothing is ever
substituted for a missing module.

For hosts calling the library directly, `stylesheet_module_hrefs(stylesheet)` returns the
hrefs as written and `transform_xml_with_modules(src, stylesheet, &modules, text_output)`
takes them pre-fetched, keyed by that href — so the transform itself stays synchronous and
I/O-free (wasm-clean). Only the **top level of the main stylesheet** is read: xrust
silently ignores a module's own `xsl:import`/`xsl:include`, so flatten nested modules by
hand.

## Caching

Both `src` (when it is an IRI) and `stylesheet` are fetched with the kernel's own
resolution path — an `http(s)://` reference goes through the HTTP module
(`urn:httpGet`), any other IRI resolves directly via `inv.source`. Either way the kernel
records each referent's **golden thread**, so the produced representation is
`.cacheable()`: it is served from cache until *either* the source or the stylesheet
changes, at which point it auto-invalidates. The transform therefore inherits the
expiry and freshness of whatever it was built from.

## Pure Rust, wasm-ready

The transform is built on [`xrust`](https://crates.io/crates/xrust) (pure-Rust XPath 1.0
/ XSLT 1.0) — no C dependency, no `libxslt`, `#![forbid(unsafe_code)]`. It runs natively
and compiles to `wasm32` unchanged; the demo lazy-loads it in the browser as a WASM
module via the sibling `ikigai-xslt-module`. The public, host-agnostic
`transform_xml(src, stylesheet, text_output) -> Result<String, String>` entry point
carries no ikigai-core types so it can be wrapped directly.

## xrust restrictions (2.1 / 2.2)

xrust implements a subset of XSLT 1.0 / XPath 1.0, and departs from the spec in places
that are easy to hit and hard to diagnose. Each of these was **verified against xrust
2.2.0** (the newest release, 2026-07-07; it differs from 2.1.0 only in item visibility
and security docs, so none of them is fixed by upgrading). Failures are silent unless
marked *error*.

| Feature | What happens | Workaround |
| ------- | ------------ | ---------- |
| `xsl:sort` under `xsl:for-each` | *error*: `unsupported XSL element "sort"` | Sort only under `xsl:apply-templates` (works). |
| `xsl:variable` inside a template | *error*: `unsupported XSL element "variable"` | Top-level `xsl:variable` only. |
| Attribute-value template with a **prefixed** name (`href="#{ik:id}"`, `{@rdf:about}`) | Attribute emitted **empty** — the AVT parser has no in-scope namespaces | `<xsl:attribute name="href">#<xsl:value-of select="ik:id"/></xsl:attribute>`; unprefixed AVTs (`{@n}`, `{.}`, `{$v}`, `{concat(…)}`) work. |
| `position()` as a value | Always `1`, in `for-each` and in matched templates | `count(preceding-sibling::*) + 1`; note `[position()=N]` as a predicate *does* work. |
| Positional predicates `[N]`, `[last()]` — on a step or on a parenthesized expression `(//x)[1]` | Silently a no-op: every node is kept | `[position()=N]` selects correctly. |
| `string-length()` | *error*: `unknown callable "string-length"` | Not available; `count()`, `concat()` etc. are. |
| Comment (or anything) before the **stylesheet's** root element | *error*: `not an XSLT stylesheet` — the first child of the document must be `xsl:stylesheet`/`xsl:transform` | Move comments inside the root. (A comment or PI before the *source* document's root is fine; a `<!DOCTYPE>` in the source is a parse error.) |
| Whitespace-only `xsl:text` (`<xsl:text> </xsl:text>`, `&#10;`, `&#160;`) and literal whitespace between two instructions | Dropped from the output | Emit a visible separator, or wrap the space with non-whitespace text (`<xsl:text>, </xsl:text>` is kept). |
| A module's own `xsl:import`/`xsl:include` | **Silently ignored** — only the main stylesheet's top level is read; a nested include neither loads nor errors | Flatten nested modules by hand. |

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
