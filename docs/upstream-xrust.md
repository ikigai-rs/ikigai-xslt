# xrust: minimal reproductions of the silent cases

Drafts for upstream reports to [xrust](https://github.com/ballsteve/xrust). **Nothing here
has been filed**: filing is an outward-facing action and is Brian's call (ledger #193, part 4).

Each case below is a stylesheet and an input that xrust evaluates **without an error** to an
answer XSLT 1.0 does not give. Since ikigai-xslt 0.3.0 the crate refuses cases 1 to 6
before xrust compiles them (`src/subset.rs`), so these drafts reproduce through stock xrust
only; case 7 is still answered. All were measured on **xrust 2.2.0** (the newest release on
crates.io, 2026-07-07) on 2026-10-10, through stock xrust (`trees::smite::RNode`, driven
exactly as the `xrust::xslt` module documentation drives it) and not through ikigai-xslt's
node wrapper. `tests/xrust_subset.rs` runs every case both ways and pins the answers, so if
any of them changes, a test fails and this file is due an update.

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
    parse(d.clone(), s, Some(|_: &_| Err(ParseError::MissingNameSpace)))?;
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
    ctxt.populate_key_values(&mut stctxt, src).unwrap();
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

## 1. A prefixed name in an attribute value template is empty

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

## 4. A numeric path predicate is ignored

Input: PLAIN.

```xml
<xsl:template match="/">
  <out><xsl:value-of select="count(doc/item[1])"/>,<xsl:value-of select="count(doc/item[last()])"/></out>
</xsl:template>
```

- **Expected** (XPath 1.0 §2.4: a number predicate means `position() = n`): `<out>1,1</out>`
- **Actual**: `<out>3,3</out>`

`doc/item[position() = 1]` gives the right answer (`count` is 1), so only the abbreviated
numeric form is affected.

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

xrust 2.2.0 has only an XML serializer (`to_xml`, `to_xml_with_options`), and its
`OutputDefinition` carries a name and an indent flag, no method. So this is a feature
request rather than a bug report. It is listed because the consequence is severe and
silent: a browser reads `<script …/>` as an open script element and the rest of the page
disappears into it.

ikigai-xslt 0.3.0 works around it: for the html method it gives each empty, non-void element
in no namespace an empty text child before calling `to_xml`, so xrust's own serializer writes
the end tag (`src/html.rs`). Void elements stay `<br/>`, not §16.2's `<br>`, and script and
style content is still escaped.

## Not reported: the refused cases

`xsl:variable` inside a template, `xsl:sort` inside `for-each`, `string-length()`, `xsl:key`
over prefixed names (`MissingNameSpace`), and a stylesheet whose first node is a comment are
all **refused with an error**. Those are gaps a stylesheet's author can see, so they are
feature requests at most, and are not drafted here.
