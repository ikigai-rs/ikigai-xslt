# xrust: minimal reproductions of the silent cases

Drafts for upstream reports to [xrust](https://github.com/ballsteve/xrust). **Nothing here
has been filed**: filing is an outward-facing action and is Brian's call (ledger #193, part 4).

Each case below is a stylesheet and an input that xrust evaluates to a wrong answer
**without an error**, or panics on instead of producing the XSLT 1.0 result. ikigai-xslt refuses cases 2 to 6 before xrust compiles them
(`src/subset.rs`), so these drafts reproduce through stock xrust only; cases 7 and 9 are still
answered. Case 10 is refused before either input is parsed (`src/limits.rs`, ledger #2519).
Case 1, and the literal `[n]` half of case 4, were first measured wrong on xrust 2.2.0
(2026-10-10) and are **fixed in 2.3.0**. Everything here was re-measured on **xrust 2.3.0**
(the newest release on crates.io, 2026-10-13) on 2026-10-14, through stock xrust
(`trees::smite::RNode`, driven exactly as the `xrust::xslt` module documentation drives it)
and not through ikigai-xslt's node wrapper. `tests/xrust_subset.rs` runs every case both ways
and pins the answers, so if any of them changes, a test fails and this file is due an update.
Case 9 was added on 2026-10-23 (ledger #2146), also measured on xrust 2.3.0.
Cases 11–13 were added on 2026-10-30 (#2520): a pre-parse refusal, boolean value-of
adaptation (or typed refusal), and a typed refusal before a serialization panic, respectively.

## The harness

The program every case runs in. It is the example from the `xrust::xslt` module
documentation, with the two documents as arguments:

```rust
use xrust::item::{Item, Node, SequenceTrait};
use xrust::parser::xml::parse;
use xrust::parser::ParseError;
use xrust::transform::context::StaticContextBuilder;
use xrust::trees::smite::RNode;
use xrust::xdmerror::{Error, ErrorKind};
use xrust::xslt::from_document;

fn doc(s: &str) -> Result<RNode, Error> {
    let d = RNode::new_document();
    parse(
        d.clone(),
        s,
        Some(|_: &_| Err(ParseError::MissingNameSpace { prefix: None, nsuri: None, message: None })),
    )?;
    Ok(d)
}

fn transform(src: &str, style: &str) -> String {
    let src = doc(src).unwrap();
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "no fetcher")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "no parser")))
        .build();
    let mut ctxt = from_document(doc(style).unwrap(), None, doc, |_| Ok(String::new())).unwrap();
    ctxt.context(vec![Item::Node(src.clone())], 0);
    ctxt.result_document(RNode::new_document());
    // xrust 2.2.0 needed `ctxt.populate_key_values(&mut stctxt, src)` here; since 2.3.0
    // `key()` indexes each document the first time it is asked.
    ctxt.evaluate(&mut stctxt).unwrap().to_xml()
}
```

Every stylesheet below is wrapped the same way; only the templates differ:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:l="urn:l#">
  <xsl:output method="xml"/>
  <!-- the templates -->
</xsl:stylesheet>
```

Two inputs:

```xml
<!-- PLAIN -->
<doc title="T"><item id="a" k="1">one</item><item id="b" k="2">two</item><item id="c" k="1">three</item></doc>

<!-- NAMESPACED -->
<page xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:l="urn:l#"><l:Item rdf:about="urn:i:1">one</l:Item><l:Item rdf:about="urn:i:2">two</l:Item></page>
```

## 1. A prefixed name in an attribute value template is empty (FIXED in xrust 2.3.0)

Kept as the record of what 2.2.0 did; xrust 2.3.0 answers the Expected line below, in every
context measured (a `for-each` body, a matched template, the root, `{count(l:Item)}`,
`{count(l:*)}`), and ikigai-xslt stopped refusing it in 0.3.1. Nothing to file.

Input: NAMESPACED.

```xml
<xsl:template match="/">
  <out><xsl:for-each select="page/l:Item"><a href="{@rdf:about}" t="{concat('x', @rdf:about)}"/></xsl:for-each></out>
</xsl:template>
```

- **Expected** (XSLT 1.0 §7.6.2): `<out><a href="urn:i:1" t="xurn:i:1"/><a href="urn:i:2" t="xurn:i:2"/></out>`
- **Actual** (xrust 2.2.0): `<out><a href='' t='x'/><a href='' t='x'/></out>`

The same template with unprefixed names works (`{@id}`, `{concat(name(), @k)}`,
`{../@title}`, `{/doc/@title}` all give the right answer on PLAIN), and the same prefixed
path works in `<xsl:value-of select="@rdf:about"/>`. So it is the name's prefix, inside an
AVT, that is lost. A possible cause, not verified: `xslt.rs` compiles a literal result
attribute with `parse_avt(…, Some(n.clone()))`, where `n` is the attribute node, so the
prefix may be resolved against the attribute rather than its element.

## 2. A predicate in a match pattern is ignored

Input: PLAIN.

```xml
<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template>
<xsl:template match="item[@k='2']"><hit><xsl:value-of select="."/></hit></xsl:template>
```

- **Expected** (§5.2; the other two items fall to the built-in rule, which copies their text):
  `<out>one<hit>two</hit>three</out>`
- **Actual**: `<out><hit>one</hit><hit>two</hit><hit>three</hit></out>`

## 3. Template priority: the first declared template wins, whatever its predicate

Input: PLAIN.

```xml
<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template>
<xsl:template match="item"><other/></xsl:template>
<xsl:template match="item[@k='1']"><one/></xsl:template>
```

- **Expected** (§5.5: `item[@k='1']` has default priority 0.5, `item` has 0):
  `<out><one/><other/><one/></out>`
- **Actual**: `<out><other/><other/><other/></out>`. With the two templates in the other
  order every item gets `<one/>`, so neither the predicate nor the priority is consulted.
  Probably the same defect as case 2, seen from conflict resolution.

## 4. `[last()]` selects the first node

Input: PLAIN.

```xml
<xsl:template match="/">
  <out><xsl:value-of select="doc/item[last()]"/></out>
</xsl:template>
```

- **Expected** (XPath 1.0 §2.4: a number predicate means `position() = n`, and `last()` is the
  context size, 3): `<out>three</out>`
- **Actual** (xrust 2.3.0): `<out>one</out>`

Probably case 5 seen from a predicate: `last()` is 1 there. A literal number works since
2.3.0 (`doc/item[2]` is `two`, `(doc/item)[2]` is `two`, `doc/item[@k='1'][2]` is `three`), and
so does a computed one (`doc/item[count(../item)]` is `three`). xrust 2.2.0 ignored every
numeric predicate (`count(doc/item[1])` was 3); that half is fixed. ⚠ A `count()` over the
result cannot show this defect, since one node is selected either way: compare values.

## 5. position() and last() are always 1 inside for-each and apply-templates

Input: PLAIN.

```xml
<xsl:template match="/">
  <out>
    <xsl:for-each select="doc/item"><p><xsl:value-of select="position()"/>/<xsl:value-of select="last()"/></p></xsl:for-each>
    <xsl:apply-templates select="doc/item"/>
  </out>
</xsl:template>
<xsl:template match="item"><xsl:if test="position() = 2"><second/></xsl:if></xsl:template>
```

- **Expected** (§7.6, the context position and size): `<out><p>1/3</p><p>2/3</p><p>3/3</p><second/></out>`
- **Actual**: `<out><p>1/1</p><p>1/1</p><p>1/1</p></out>`

(Whitespace between the elements above is for reading; the measured stylesheet has none.)

## 6. A leading // is evaluated from the context node, not the root

Input: PLAIN.

```xml
<xsl:template match="/"><out><xsl:apply-templates select="doc/item"/></out></xsl:template>
<xsl:template match="item"><p><xsl:value-of select="count(//item)"/></p></xsl:template>
```

- **Expected** (XPath 1.0 §2.5: `//` is `/descendant-or-self::node()/`, from the root of
  the context node's document): `<out><p>3</p><p>3</p><p>3</p></out>`
- **Actual**: `<out><p>0</p><p>0</p><p>0</p></out>`

From inside that template `count(//node())` is 2, the `item` and its text, which is what
`descendant-or-self::node()` of the context node gives; `count(/doc//item)` is 3. From the
root template `count(//item)` is 3 only because the context node there IS the root.

## 7. xsl:value-of over several nodes concatenates them all

Input: PLAIN.

```xml
<xsl:template match="/"><out><xsl:value-of select="doc/item"/></out></xsl:template>
```

- **Expected** (XSLT 1.0 §7.6.1: a node-set is converted with `string()`, which takes the
  first node in document order; an XSLT 2.0+ processor running a `version="1.0"` stylesheet
  in backwards-compatible mode does the same): `<out>one</out>`
- **Actual**: `<out>onetwothree</out>`. That is neither the 1.0 answer nor the 3.0 one
  (`one two three`, separated by a space).

The same conversion happens wherever several nodes become a string: `string-length(doc/item)`
is 11 (`onetwothree`), where XSLT 1.0 gives 3 (`one`).

## 8. Not a defect, a missing feature: xsl:output method="html"

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="html"/>
  <xsl:template match="/"><html><head><script src="a.js"></script></head><body><textarea></textarea><br/></body></html></xsl:template>
</xsl:stylesheet>
```

- **Expected** (XSLT 1.0 §16.2, the HTML output method): `<script src="a.js"></script>`,
  `<textarea></textarea>`, `<br>`
- **Actual**: `<html><head><script src='a.js'/></head><body><textarea/><br/></body></html>`

xrust 2.3.0 has only an XML serializer (`to_xml`, `to_xml_with_options`), and its
`OutputDefinition` carries a name and an indent flag, no method. So this is a feature
request rather than a bug report. It is listed because the consequence is severe and
silent: a browser reads `<script …/>` as an open script element and the rest of the page
disappears into it.

ikigai-xslt 0.3.0 works around it: for the html method it gives each empty, non-void element
in no namespace an empty text child before calling `to_xml`, so xrust's own serializer writes
the end tag (`src/html.rs`). Void elements stay `<br/>`, not §16.2's `<br>`, and script and
style content is still escaped.

## 9. Whitespace-only xsl:text is dropped

Input: `<doc/>`.

```xml
<xsl:template match="/"><out>Meta<xsl:text> </xsl:text>Source</out></xsl:template>
```

- **Expected** (XSLT 1.0 §3.4 and §7.2: whitespace inside `xsl:text` is preserved):
  `<out>Meta Source</out>`
- **Actual**: `<out>MetaSource</out>`, without an error, through both stock xrust 2.3.0
  and ikigai-xslt. Two spaces, a tab followed by a newline, and `&#x20;` are also lost.

Nonblank text such as `<xsl:text>Meta </xsl:text>` retains its trailing space. Both
`<xsl:value-of select="' '"/>` and `<xsl:value-of select="concat(., ' ')"/>` preserve the
space and are tested workarounds. ikigai-xslt records this limitation in its subset table;
it does not rewrite or refuse these stylesheets.

Likely cause, from inspecting xrust 2.3.0: `xslt::from_document` calls
`transform::whitespace::strip_whitespace` with a wildcard strip rule and a preserve rule
for `xsl:text`. In `strip_whitespace_node`, the preserve-rule branch assigns matching
scores to `ss` (the strip score), leaving `ps` (the preserve score) at -1. Therefore the
`xsl:text` children are visited with `keep = false`, and whitespace-only text is removed
before the instruction compiles. The upstream fix should assign preserve scores to `ps`
and test that stylesheet indentation still strips while `xsl:text` whitespace survives.
This is an upstream report draft; nothing has been filed.

## 10. A prefixed attribute's value is lowercased by the XML parser

Input: an RDF/XML-shaped source whose IRI has capitals in it.

```xml
<page xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:l="urn:l#"><l:Item rdf:about="urn:I:One" plain="urn:I:One">x</l:Item></page>
```

```xml
<xsl:template match="/"><out><xsl:value-of select="page/l:Item/@rdf:about"/></out></xsl:template>
```

- **Expected** (XML 1.0 §3.3.3 normalizes an attribute value's whitespace, never its case;
  Namespaces in XML does not touch values): `<out>urn:I:One</out>`
- **Actual**: `<out>urn:i:one</out>`, without an error, through both stock xrust 2.3.0 and
  ikigai-xslt before its #2519 guard. This crate now refuses the input before parsing.
  The unprefixed `plain` attribute, with the same text, keeps its case:
  `<out>urn:I:One</out>`. So the defect is in the prefixed path, not in attribute values in
  general.

This is the most damaging of the silent cases for RDF: `rdf:about`, `rdf:resource` and
`rdf:datatype` are all prefixed, so any IRI with a capital letter in it becomes a different
IRI. The same happens to a prefixed attribute written literally in a stylesheet, and to
`xml:id`.

Cause, from reading xrust 2.3.0's `src/parser/xml/attribute.rs`: the loop over the parsed
attributes matches on `(prefix, local_part, value.to_lowercase())` so that `xml:space` can
be recognized case-insensitively as `default` or `preserve`. But every arm binds that
lowercased value, and the generic prefixed arm `(Some(p), lp, v)` stores `v` as the
attribute's value (the `xml:id` arm does the same). The fix is to match and store the
value as parsed in every arm, validating `xml:space` case-sensitively, with a test that
`rdf:about="urn:I:One"` reads back unchanged. (A data-driven value, one computed in the
stylesheet rather than read from a prefixed attribute, keeps its case, which is the
workaround ikigai-xslt's callers use today.)

The defective match is at **line 233** in xrust 2.3.0; the generic prefixed arm
at lines 303–328 stores its lowercased `v`. The one-line fix is:

```diff
-                        value.to_lowercase().as_str(),
+                        value.as_str(),
```

This also makes `xml:space` accept only the case-sensitive XML values `default` and
`preserve`. The same bug affects stylesheet literals:

```xml
<xsl:template match="/">
  <rdf:v rdf:datatype="http://www.w3.org/2001/XMLSchema#dateTime"/>
</xsl:template>
```

With RDF and XSL namespaces declared as above, stock xrust produces
a `rdf:v` element whose `rdf:datatype` is `http://www.w3.org/2001/xmlschema#datetime`.
The datatype must retain `XMLSchema#dateTime`. `tests/xrust_subset.rs` pins both stock wrong answers and this
crate's refusals, plus the computed-value workaround. This draft is not filed upstream.

## Not reported: the refused cases

`xsl:sort` inside `for-each` or `for-each-group` is **refused with an error**. That is a gap a
stylesheet's author can see, so it is a feature request at most, and is not drafted here.
(`xsl:variable` inside a template, `string-length()`, `xsl:key` over prefixed names and a
stylesheet whose first node is a comment were refused by 2.2.0 and work in 2.3.0.)

## 11. Attribute whitespace character references normalize to spaces (#2520)

Measured on xrust 2.3.0, 2026-10-30. Source `<doc/>`; stylesheet body:

```xml
<xsl:template match="/">
  <out><xsl:value-of select="concat('a', '&#10;')"/></out>
</xsl:template>
```

Expected `<out>a\n</out>` (a literal LF); actual `<out>a </out>`. Decimal, hexadecimal,
and leading-zero forms reproduce it, as do tab and CR references. It also affects source
attributes: `<doc v="a&#10;b"/>` read with `doc/@v` yields `a b` instead of `a\nb`.
XML 1.0 §3.3.3 preserves character references; only literal attribute whitespace normalizes.

Cause: `src/parser/xml/attribute.rs:401–448` collects raw text and expanded references into
one vector, then `rn.concat().replace(...)` normalizes both; `split_whitespace()` also
collapses and trims CDATA values. Suggested patch: normalize literal text chunks at the
`take_while` branch (lines 405 and 420), preserve `textreference()` results, then concatenate
without the final whitespace replacement/collapse for CDATA attributes. Non-CDATA attribute
normalization must remain a separate step. This needs XML parser conformance tests.

ikigai-xslt refuses these references in **either input before parsing**, naming the attribute
and argument. An exact repair after parsing cannot distinguish a reference from a literal
space. RSS workaround (tested with text output): declare a variable containing dot, literal
newline, dot; obtain the LF with `substring($lf-holder, 2, 1)`.

## 12. Boolean string conversion is empty (#2520)

Source `<doc/>`; stylesheet body:

```xml
<xsl:template match="/">
  <out><xsl:value-of select="1 &gt; 0"/></out>
</xsl:template>
```

Expected `<out>true</out>`; actual `<out></out>`. `1 &lt; 0`, `true()`, `false()`,
`boolean(...)`, `not(...)`, comparisons and boolean string functions reproduce it.
`xsl:if` works because effective boolean value is implemented. `string(true())` is empty too.

Cause: `src/value.rs:219–268`, `impl Display for ValueData`, has no `Boolean` arm and falls
through to `_ => ""`. `src/transform/construct.rs:150` uses the sequence's string value for
`xsl:value-of`; `src/transform/strings.rs:203–205` uses the same conversion for `string()`.
The obvious upstream patch, before the fallback in `ValueData::fmt`, is:

```rust
ValueData::Boolean(b) => b.to_string(),
```

ikigai-xslt adapts known boolean `value-of` results to `xsl:choose` with literal `true` and
`false` text. It follows variable declarations; results whose type cannot safely be known,
including boolean-default parameters that may be overridden by a string, are typed refusals.
The general upstream string conversion defect remains outside this direct-result adaptation.

## 13. A literal prefixed attribute can panic in serialization (#2520)

Source `<doc/>`; stylesheet declares `rdf` only on `xsl:stylesheet`:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <xsl:template match="/"><out><b rdf:datatype="x">1</b></out></xsl:template>
</xsl:stylesheet>
```

Expected an XML result with a namespace declaration for the `rdf:datatype` attribute.
Actual: serialization panics `unable to find namespace`. Declaring `xmlns:rdf` directly on
`b` does not help. Changing `out` to `rdf:RDF` works.

Cause: `src/xslt.rs:1453–1469` emits namespace declarations only for a literal element's
own name, then copies attributes without their required namespace declarations.
`src/trees/smite.rs:1255–1267` resolves the attribute name against the result's in-scope
namespaces and panics when no namespace is found. Suggested compiler patch: emit declarations
for literal attributes' namespaces too (or copy the applicable in-scope namespaces, respecting
XSLT exclusion rules), rather than only the element namespace. The serializer should also
return an error for an unresolvable QName instead of panicking; that needs an API change.

ikigai-xslt refuses this before compilation as a typed `InvalidArgument` naming `stylesheet`.
A literal element's own namespace, or an enclosing literal element in that namespace, satisfies
the guard. Template calls and dynamically constructed namespace scopes are treated
conservatively; the guard does not assume that variable/parameter fragments are emitted at their declaration site. The stock panic
and the admitted `rdf:RDF` workaround are both pinned in `tests/xrust_subset.rs`.
