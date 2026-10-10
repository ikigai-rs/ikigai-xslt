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

### `generate-id()` is a position, so the answer is the same in every process

xrust answers `generate-id()` with the node's heap address (`0x00000076eb01c790`), which
differs from process to process: a cached answer was not a function of its inputs, the
stylesheet's author learned where the heap is, and the value was not the "alphanumeric,
starting with a letter" string XSLT promises. From 0.2.1 (ledger #1047) this crate runs xrust
over its own node type, which answers the node's **path in its tree** instead, in letters and
digits:

| node | id |
| --- | --- |
| the source document | `d1` |
| its document element | `d1c1` |
| that element's 2nd child (any kind of node) | `d1c1c2` |
| its 1st attribute (in name order) | `d1c1c2a1` |
| the top of any other tree, numbered as first asked | `d2`, `u3` (not in a document) |

The same node always has the same id within a transform and two nodes never share one, as
XSLT requires; across documents an inserted node renumbers its later siblings.

## Caller XML is bounded, and cannot take the process down

xrust parses, compiles and evaluates by recursion, and a stack overflow aborts the whole
process. Measured on a 2 MiB thread (a tokio worker's) through 0.2.0: 24 nested source
elements aborted a debug build and 114 a release one, and an XPath nesting 13 parentheses —
about 40 bytes of stylesheet — aborted a release build (7 in debug). Both inputs are caller
XML wherever a host offers the endpoint, so from 0.2.1 (ledger #916):

| bound | value | refused as |
| --- | --- | --- |
| element nesting, either input (`limits::MAX_XML_DEPTH`) | 64 | `InvalidArgument` naming `src`, `content` or `stylesheet` |
| bracket nesting in one stylesheet attribute value — XPath and `{…}` templates (`limits::MAX_EXPR_DEPTH`) | 32 | `InvalidArgument` naming `stylesheet` |
| text the declared entities expand to (`limits::MAX_ENTITY_EXPANSION`) | 1 MiB | `InvalidArgument` |

The check is one non-recursive scan before xrust sees a byte; comments, CDATA sections and
processing instructions are skipped exactly. A `<!DOCTYPE` internal subset is read: an
entity whose value holds `<`, `&` or `%` (markup, a reference, or a character reference that
becomes markup), a parameter entity, and an external entity are refused, because xrust
expands entities as content and the tag scan cannot see what they build. The declaration
RDF/XML uses — a name for a namespace IRI — is admitted. No external DTD is ever fetched.

Then the transform runs on a **pooled thread whose stack holds anything the bounds admit**
in that build (`limits::xslt_stack_size()`: ~45 MiB release, ~338 MiB debug — reserved
address space, committed only as it is touched), so a call from a 2 MiB async worker is as
safe as any other. That covers what no lexical bound can: a template that calls itself
nests its result once a call, up to xrust's own 200 calls. The threads are kept (up to 8
idle) because the compiled-stylesheet memo below lives on them. On wasm there is no thread,
so only the bounds apply there.

Real input is far inside the bounds: gonk's 98 KB stylesheet nests 14 elements, the reading
room's stylesheets 5 to 9, and their expressions two or three brackets. Neither layer bounds
**time**; the next section does.

## A transform is answered within a time budget

Some shapes inside every bound above are super-linear in xrust. Measured in a release build
(`cargo run --release --example time-cost -- <gonk.xsl> <cms-web styles dir>`, 2026-10-10):

| input | time |
| --- | --- |
| gonk's 98 KB stylesheet, a 10-row queue chunk (what gonk renders), cold compile included | 0.73 s |
| the reading room's `catalog.xsl`, a 60-resource page | 0.045 s |
| an XPath of 16,384 `+1` terms (32 KB of stylesheet, cold) | 1.9 s |
| a template recursing to build a 1,592-level result (~600 bytes of stylesheet) | 3.7 s |

and the recursion grows about as the cube of the result's depth, which xrust lets reach 199
calls of up to 60 nested elements each: minutes, from under 1 KB. So from 0.2.1 (ledger #1040)
every transform this crate starts — the endpoint, `transform_xml`, `stylesheet_output_method`
— is answered within **`limits::DEFAULT_TIME_BUDGET`, 5 s** (the same base as the
ecosystem's SPARQL budgets), with a typed `Timeout` naming the budget past it and never a
partial result. A host that renders larger documents on purpose calls
`transform_xml_within(src, stylesheet, text_output, budget)`. The endpoint has no host
constructor and no argument for a budget, so its budget is the constant.

⚠ **The work is abandoned, not cancelled.** xrust has no cancellation point, and a Rust
thread cannot be stopped from outside, so a transform past its deadline runs to its end on
its own thread. The caller is still answered on time, and the CPU is bounded by a count: while
`limits::max_overdue_transforms()` (a quarter of the machine's cores, at least one) are still
running after their callers were answered, every new transform is refused at once with a
transient `Unavailable`, until one ends — `limits::overdue_transforms()` says how many there
are. That trades XSLT availability for the host's other work, as `ikigai-store` does for
SPARQL. On wasm there is no thread, so there is no deadline either.

**A panic inside xrust is an error, not a panic.** xrust 2.2.0 panics on some input a caller
controls — `<xsl:copy-of select="/"/>`, an attribute or the document node at the top of the
result, an `xsl:sort` key that fails to evaluate — and through 0.2.0 that panic reached the
caller's thread. It is now caught on the transform thread and answered as an `Endpoint` error
naming it (`endpoint error:` from `transform_xml`). The default panic hook still prints the
message to stderr; that hook is the host's.

## Compiling the stylesheet is the cost, and it is reused

Almost all of an XSLT call is parsing and compiling the **stylesheet**, which has nothing
to do with the document being styled. Measured on gonk's 38 KB, 56-template stylesheet
(`cargo run --release --example stylesheet-cost -- <stylesheet.xsl> [source.xml]`):

| | cost |
| --- | --- |
| parse the stylesheet | ~34 ms |
| compile the parsed tree | ~119 ms |
| ready a compiled stylesheet for one run | ~0.012 ms |
| run a real page through it | ~3.7 ms |

Through 0.1.2 that whole ~156 ms was paid on **every** call, so an empty document cost
157 ms and a real page 163 ms — fixed overhead, identical whatever the data, and with no
warm-up between calls. From 0.1.3:

* **`CompiledStylesheet::compile(stylesheet)`** parses and compiles once and
  `.transform(src, text_output)` runs any number of documents against it, each run
  independent of the last. It also carries `.output_method()`, so labelling a result no
  longer costs a second parse of the stylesheet.
* **`transform_xml` keeps its exact signature** and memoizes the compile per thread
  (since 0.2.1, per pooled transform thread), keyed on the stylesheet's full text. Existing callers get the saving with no change:
  the empty document goes 157 ms → **0.14 ms**, a real gonk page 163 ms → **3.7 ms**.

The memo is keyed on the bytes the caller just passed — not on a path, an IRI or a
timestamp — so an edited stylesheet is simply a different key and there is nothing that
can go stale under the kernel's golden threads. It holds four entries per thread
(~369 KB each for a stylesheet this size); `clear_stylesheet_cache()` drops them (and lets
the idle pooled threads go), and changes no answer.

⚠ A compiled stylesheet is **`!Send` and `!Sync`**, and cannot be otherwise: xrust's tree
is `Rc`-based. A multi-threaded host holds one per thread (which is what `transform_xml`
does for you), per connection or per task — never in shared state. And it runs on the
caller's thread: `compile` and `transform` enforce the bounds, but the stack they need is
the caller's to provide (`limits::xslt_stack_size()`, or run inside
`limits::on_xslt_stack`). `transform_xml` does that for you too.

## Conformance

**Passes [`ikigai-conformance`](https://github.com/ikigai-rs/ikigai-conformance)** with
no opt-outs on the module's endpoint: `tests/conformance.rs` walks `urn:xslt:transform`
and runs every check — ArgSpecs, declared = enforced, the RDF faces (none: the output is
the stylesheet's, not a graph this module authors), the cacheable-twice probe, pipeline
citizenship, naming, and the space's name (`space()` is declared self-named). The one
waiver is on the test's own stand-in files, which model a watched `ikigai-fs` file and
are cut by the test rather than by a write through their name. Three
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
`transform_xml(src, stylesheet, text_output) -> Result<String, String>` entry point —
and `CompiledStylesheet`, which has the same plain-string errors — carry no ikigai-core
types, so they can be wrapped directly.

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

`space()` is configuration-free, so it names itself `urn:iki:space:xslt`
(`ikigai_xslt::SPACE_ID`), which is how `urn:kernel:topology`, `answered_by` and the space
diagrams show it. Binding another door onto it drops the name.

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
