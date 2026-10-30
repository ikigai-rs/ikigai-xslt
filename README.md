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

A missing input is refused **typed** (since 0.3.1): no `stylesheet` is a
`MissingArgument` naming `stylesheet`, and neither `src` nor `content` is a
`MissingArgument` naming `src`. An input that is present but not inline UTF-8 is an
`InvalidArgument` naming that input; a bad `src` does not fall through to `content`.

Every fixed input is declared with an XSD class (`xsd:string` for all four: `src` and
`stylesheet` are each a union of an IRI and a document, which no ArgSpec class states
more precisely), so the manifold and `urn:kernel:validate` can form and check a call.

## Stylesheet parameters

Every argument other than `src`, `stylesheet`, `content` and `as` supplies a **string**
value to a declared top-level `xsl:param` with the same name. For example:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:param name="feed"/>
  <xsl:param name="terms" select="'All items'"/>
  <xsl:template match="/">
    <rss><channel xmlUrl="{$feed}"><title><xsl:value-of select="$terms"/></title></channel></rss>
  </xsl:template>
</xsl:stylesheet>
```

```shell
source urn:xslt:transform src=urn:data:feed.xml stylesheet=urn:style:search.xsl feed=https://example.org/feed.xml terms="Rust and XML"
```

Values must be inline UTF-8, just like the fixed inputs. They are literal strings,
never XPath expressions or resource references: `terms=1+2` passes the text `1+2`.
An omitted parameter uses its stylesheet default (`select` or content); an explicit
empty string overrides that default. Defaults and top-level variables can use the
selected parameter values. Request arguments already contribute to cache identity;
compilation is still memoized by stylesheet text, with fresh parameter bindings per run.

The endpoint declares an **open argument set** so named requests can carry these names.
An undeclared name, a local template parameter or an `xsl:variable` is refused as an
`InvalidArgument` naming that argument. The four fixed names remain reserved. Parameter
names must be unprefixed XML names; xrust resolves variable references by local name,
so namespace-qualified top-level parameter names are refused as an `InvalidArgument`
naming `stylesheet`, as are missing, invalid or duplicate parameter names.

## Output media type

The result is labeled by `as=` when given; otherwise by what the stylesheet's top-level
`xsl:output method` implies — the three declared outputs:

| `xsl:output method` | media type | serialization |
| ------------------- | ---------- | ------------- |
| `html` (or no `xsl:output`) | `text/html` | HTML (since 0.3.0; see [below](#methodhtml-is-serialized-as-html)) |
| `xml` | `application/xml` | XML |
| `text` | `text/plain` | the result's string value, whitespace preserved, nothing escaped |

A `method="text"` stylesheet is serialized as text whatever `as=` says, and `as=text/plain`
selects text serialization for any stylesheet. `as=` may relabel the markup with a type the
declaration does not list (`image/svg+xml` for an SVG-emitting stylesheet); the declared
three are what the endpoint chooses by itself.

`document()` is not supported: the transform runs synchronously and a kernel resolution
does not, so the engine's fetcher refuses every URL. A stylesheet that calls it fails with
a typed `Endpoint` error naming the function, and the referenced resource is never
resolved — there is no read for a capability to gate.

## The XSLT subset: what xrust runs, refuses, and gets wrong silently

xrust implements a **subset** of XSLT 1.0, and the dangerous part of it is not what it
refuses: it is what it evaluates **wrongly, with no error**. A stylesheet's author cannot
discover those from errors, so since 0.3.0 this crate **refuses** the constructs in the refusal table
below, before xrust compiles the stylesheet. Every row below was measured on
**xrust 2.3.0** (2026-10-14, ledger #1492; first on 2.2.0, ledger #193;
whitespace-only `xsl:text` added 2026-10-23, ledger #2146; attribute whitespace references,
boolean conversion and literal attribute namespaces added 2026-10-30, ledger #2520) by
`tests/xrust_subset.rs`, and each wrong answer is
also reproduced against stock xrust, without this crate in the way, in
[`docs/upstream-xrust.md`](docs/upstream-xrust.md). Those tests pin stock xrust's wrong
answers, so the day xrust fixes one, a test fails and says so: that is the signal to drop
the refusal and update this table. The measured release is a record
(`ikigai_xslt::subset::XRUST_MEASURED_ON`), not a gate: a newer xrust on crates.io is a
warning in the ecosystem's daily scan, not a red build.

**Works**: `xsl:if`, `xsl:choose`, `xsl:attribute` (prefixed names included),
`call-template`, modes, `apply-templates` with `xsl:sort`, `count()`, `string-length()`, a
`[@attr = '…']`, `[position() = n]` or literal `[n]` filter on a `select` path (`item[2]`,
`(…)[2]`, `item[@k='1'][2]`), attribute value templates, prefixed names included (`{@id}`,
`x-{@id}`, `{concat(name(), @k)}`, `{../@title}`, `{/doc/@title}`, `{@rdf:about}`), an
absolute `/doc/…` path from inside a nested template, `xsl:variable` at the top level and
inside a template, `xsl:key` over unprefixed and prefixed names, `xsl:comment`, and a
stylesheet document that begins with a comment. `{@rdf:about}` resolves the prefixed
**name**; prefixed attribute **values** with uppercase letters are refused as described below.

**Changed in 0.3.1, with xrust 2.3.0** (ledger #1492): a literal `[n]` predicate, a prefixed
name in an attribute value template, `xsl:variable` inside a template, `string-length()`,
`xsl:key` over prefixed names and a stylesheet that begins with a comment all give the XSLT 1.0
answer now, so the first two are no longer refused and the last four moved here from the table
below. `xsl:sort` inside
`xsl:for-each-group` is newly refused by xrust when it compiles the stylesheet.

**Refused with an error** (the safe failure):

| construct | error |
| --- | --- |
| `xsl:sort` inside `xsl:for-each` or `xsl:for-each-group` (inside `apply-templates` works) | `unsupported XSL element "sort"` |

**Refused before it compiles** (since 0.3.0): xrust 2.3.0 answers these wrongly or panics,
so this crate refuses a stylesheet holding one, as an `InvalidArgument` naming
`stylesheet` that quotes the attribute, its element and the row below
(`invalid argument `stylesheet`: refused: match="item[@k='2']" on <xsl:template> holds "a
predicate in a match pattern", …`). `ikigai_xslt::subset::silent_constructs(stylesheet)`
lists these constructs at once, and `cargo run --example subset-scan -- file.xsl` does the same from
a shell.

| construct | XSLT 1.0 answer | xrust 2.3.0 answer | instead, write |
| --- | --- | --- | --- |
| a predicate in a match pattern: `match="item[@k='2']"`, in `xsl:template` or `xsl:key` | the items whose `k` is 2 | **every** `item`; and with two templates for one name, the **first declared** wins whatever its priority | `match="item"` and an `xsl:if` / `xsl:choose` inside it |
| a [last()] predicate — `item[last()]`, `(…)[last()]`, `{item[last()]/@id}` | the last node | the **first** node, since `last()` is 1 | `item[count(../item)]` |
| position() or last() away from the root: inside `for-each`, `apply-templates`, a named template; and `last()` inside any predicate | 1, 2, 3 … / the count | always **1**, so `test="position() = 2"` is never true | compute it outside and hand it in as data; `position()` inside a predicate works |
| a leading // away from the root: `count(//item)` inside a nested or named template, a `for-each` body or a predicate | from the document root | from the **context node**, so usually nothing | `/doc//name` |
| a literal attribute without a result namespace declaration: `<out><b rdf:datatype="x">1</b></out>`, with `rdf` declared on the stylesheet | a namespaced attribute | serialization panics: `unable to find namespace`; xrust only emits namespace declarations for element names | put the element or an enclosing literal result element in that namespace, e.g. `<rdf:RDF>` |

"Away from the root" is everywhere but the body of `xsl:template match="/"` outside any
`xsl:for-each`, and a top-level `xsl:variable` or `xsl:param`: there the context node is the
root, xrust's answer is right, and nothing is refused. The scan is lexical and
conservative: a `[`, `//` or `prefix:name` inside a string literal is text, `a//b` and
`/doc//b` are not a leading `//`, and `[1]` or `[position() = 1]` is not a `[last()]` predicate.

**Refused before parsing either input** (ledger #2519): a prefixed attribute whose
value would change under Unicode lowercasing, in the source **or** stylesheet. This includes
`rdf:about="https://Example.org/My-Post"`, `rdf:datatype="http://www.w3.org/2001/XMLSchema#dateTime"`,
`xml:id`, `xml:base`, and uppercase variable names inside a prefixed attribute's
value template (`f:k="{$camelVar}"`). Stock xrust lowercases these values; this crate returns
`InvalidArgument` naming `src` or `stylesheet`, the attribute, and the defect. Character
references and declared entity values are checked too. Namespace declarations (`xmlns:f`)
are exempt because xrust preserves their values. A literal `xml:lang` is exempt too: a
language tag is case-insensitive (BCP 47), so the lowercased `en-us` xrust keeps means what
`en-US` meant. An `xml:lang` value template is still refused.

| construct | XML/XSLT answer | stock xrust 2.3.0 answer | instead, write |
| --- | --- | --- | --- |
| a prefixed attribute with uppercase letters in its parsed value, in either input | preserve case | lowercase the value, silently changing IRIs and AVT variable names | carry the value in an unprefixed attribute, element text, or a parameter; compute the output with `xsl:attribute` or `rdf:about="{$iri}"` using a lowercase variable name |
| a tab, line feed or carriage return character reference in any attribute (`&#9;`, `&#10;`, `&#13;`, including hexadecimal and leading-zero forms) | retain the referenced character | replaces it with a space | for the RSS newline, declare `<xsl:variable name="lf-holder">.` followed by a literal newline and `.</xsl:variable>`, then use `substring($lf-holder, 2, 1)` |

The whitespace-reference refusal (#2520) also applies to **unprefixed** attributes in either
input, before parsing, and names the argument and attribute. There is no exact repair after
parsing: a reference and a literal space have already become indistinguishable. Text-node
references, escaped `&amp;#10;`, comments, CDATA and processing instructions are unaffected.
The RSS workaround above produces one line per item with `method="text"` (tested).

Lowercase prefixed values pass the case guard; the literal result namespace guard still
applies. Unprefixed values and values computed from data keep their case. This guard also applies to `CompiledStylesheet::compile` and `transform`.

**Numeric conversion (#2487, since 0.5.0):** `number(/r/v)` and arithmetic on
selected nodes or `string-length()` results work through this crate, including in chart
attribute value templates such as `cx="{/r/v + 1}"`. Stock xrust 2.3.0 returns `NaN` for
these operands. Before compilation, this crate converts each numeric operand through the
string value of its first item, trimming XML whitespace; empty selections and nonnumeric
text yield `NaN`. `number()` without an argument uses the context node. XPath 1.0 operator
precedence, predicates, variable references, names and string literals are preserved.
Syntax outside this adapter's XPath 1.0 grammar is left to xrust. Generated expressions
obey `MAX_EXPR_DEPTH` too: a near-limit expression may be refused because the conversion
adds brackets. The adapter addresses node and unsigned numeric conversion, not xrust's
separate boolean-to-string defect described below. `tests/numeric_conversion.rs` covers
the public path; `tests/xrust_subset.rs` pins the unadapted upstream failures.

**Boolean `xsl:value-of` (#2520):** this crate converts known boolean results to explicit
`true` / `false` text with `xsl:choose`, preserving the expression's context and evaluating it
once. It reads xrust's parsed XPath, so comparisons inside predicates and string literals do
not trigger this adaptation. Boolean variables are followed through their lexical declarations,
up to `MAX_EXPR_DEPTH` bindings; longer or cyclic chains are refused. A template-local
parameter combines its default with every `with-param` from calls to its named template
and, conservatively, every `apply-templates` passing that parameter name. String-only
parameters remain accepted; consistently boolean parameters are adapted; mixed or unknown
types are refused. Global parameters retain the request-argument rule: strings can override
the default, so a boolean default is refused.

| construct | XSLT 1.0 answer / this crate | stock xrust 2.3.0 answer | handling |
| --- | --- | --- | --- |
| `<xsl:value-of select="1 &gt; 0"/>` (also `false()`, comparisons and boolean functions) | `true` (or `false`) | empty string for either boolean | adapted before compilation |
| `xsl:value-of` of a template-local parameter whose default and all call-site values are nonboolean | the supplied/default string or node value | correct | accepted; pinned with CMS links, gonk, and RSS stylesheet fixtures |
| `xsl:value-of` of a template-local parameter whose default and all call-site values are boolean | `true` / `false` | empty string | adapted before compilation |
| `xsl:value-of` of a global boolean-default parameter, a local parameter with mixed boolean/nonboolean incoming values, or another expression whose result type cannot be established | explicit refusal here | a boolean becomes empty | typed `InvalidArgument` naming `stylesheet`; write `xsl:choose` with explicit true/false text, or supply a string |

Only the direct boolean result of `value-of` is adapted; xrust's general boolean-to-string
conversion (for example `string(false())`) remains defective. A literal attribute namespace
must be supplied by its own or an enclosing literal element's **name**, even if `xmlns:rdf`
is written on that element: xrust does not copy that declaration by itself. The guard is
conservative across dynamic element construction and matched-template dispatch. Named-only
templates are accepted when every call site supplies the namespace through a literal
result ancestor (following at most `MAX_EXPR_DEPTH` template calls). A variable/parameter
fragment can be emitted elsewhere, so the guard does not count its outer literal ancestors.

⚠ **Still silently wrong: an answer, no error.** These cases are recorded limitations;
they are not refused here. Whether a path selects one node or several is a property of
the data, not of the stylesheet (`ik:title` is usually one node; `../@title` always is;
`item` often is not). Whitespace-only `xsl:text` is lost during stylesheet compilation,
including a space written as `&#x20;`:

| construct | XSLT 1.0 answer | xrust 2.3.0 answer | instead, write |
| --- | --- | --- | --- |
| `xsl:value-of` over several nodes, and any other place several nodes become a string (`string-length(item)`) | the **first** node's string value | **all** of them concatenated, no separator (`string-length(item)` is 11 for `one`, `two`, `three`) | `select="item[1]"` |
| `xsl:text` containing only whitespace: `Meta<xsl:text> </xsl:text>Source` | `Meta Source` | `MetaSource` (the explicit whitespace is dropped) | `<xsl:value-of select="' '"/>`, or append a space with `concat(., ' ')` |

### `method="html"` is serialized as HTML

xrust has only an XML serializer, so through 0.2.x every empty element came out
self-closed (`<script src='a.js'/>`, `<textarea/>`), and a browser reads `<script …/>` as an
**open** script element that swallows the rest of the page. Since 0.3.0 a `method="html"`
result (or, with no `xsl:output`, one whose first element is `html`, XSLT 1.0's default) is
serialized as HTML, by `CompiledStylesheet::transform`, `transform_xml` and the endpoint
alike:

- an empty element HTML does not call void keeps its end tag: `<script src='a.js'></script>`,
  `<textarea></textarea>`, `<div></div>`;
- a void element (`br`, `img`, `input`, `meta`, `link`, … compared ignoring case) has none:
  `<br/>`. The trailing slash is xrust's and HTML ignores it on a void element, which also
  makes the output byte for byte what gonk's own pass wrote (`ikigai-gonk/src/render.rs`);
- an element in a namespace (inline SVG) is XML, as XSLT 1.0 §16.2 says: `<path/>`.

Everything else (escaping, quoting, namespace declarations) is xrust's, unchanged. ⚠ One
part of §16.2 is not done: `script` and `style` content is still escaped, so an INLINE script
with a `'` or a `<` in it breaks (`'` becomes `&apos;`). Writing it raw would make any data a
stylesheet copies into a script executable markup, so it stays a host's decision; keep scripts
and styles in their own files.

What the gaps cost a real stylesheet, and how one is written around them, is measured in
gonk's renderer: everything a template would compute arrives as data on the node it
describes instead.

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

**A panic inside xrust is an error, not a panic.** xrust panics on some input a caller
controls — `<xsl:copy-of select="/"/>`, an attribute or the document node at the top of the
result, an `xsl:sort` key under `apply-templates` that fails to evaluate (2.3.0 still does all
of these). The literal prefixed attribute case described in the subset table is now refused
before compilation as `InvalidArgument`, before the serializer can panic. Through 0.2.0 that panic reached the
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
citizenship, required inputs refused typed, naming, and the space's name (`space()` is declared self-named). The one
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
/ XSLT 1.0, a subset of it: see [above](#the-xslt-subset-what-xrust-runs-refuses-and-gets-wrong-silently)) — no C dependency, no `libxslt`, `#![forbid(unsafe_code)]`. It runs natively
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
