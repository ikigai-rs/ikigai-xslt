# Upstream draft: `number()` and arithmetic on a node, or on `string-length()`, give NaN

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-numeric-nodes.patch`](xrust-numeric-nodes.patch). **Nothing here has been filed**:
filing is Brian's call (ledger #2487, #3343). ikigai-xslt carries the same fix in
`crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one's test, added to `tests/smite-xslt.rs`, fails on xrust 2.3.0 (`NaN` for
`number(/r/b/l)`) and passes with the fix (checked 2026-11-09). Check GitLab's `main`
before filing.

---

**Title:** `number(node)`, `node + 1` and `string-length('abc') - 1` are NaN

Source `<r><b n=" 7 "><l>12.5</l><l>4</l></b></r>`:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:value-of select="number(/r/b/l)"/></out></xsl:template>
</xsl:stylesheet>
```

- Expected (XPath 1.0 §4.4: a node-set converts by the string value of its first node,
  with leading and trailing white space ignored; an empty one is NaN): `<out>12.5</out>`
- Actual (2.3.0): `<out>NaN</out>`. `/r/b/l + 1` is NaN (expected 13.5), `/r/b/@n * 2`
  is NaN (expected 14), and `string-length('abc') - 1` is NaN (expected 2).
  `number(string(/r/b/l))` works.

Causes:

- `Item::to_double` in `src/item.rs` returns NaN for every node.
- `Value::to_double` in `src/value.rs` handles strings, `Integer`, `Int` and `Double`
  only, so the `UnsignedLong` that `string-length()` returns is NaN; and it parses a
  string without ignoring surrounding white space.
- `transform::numbers::number` refuses a sequence of more than one item ("not a singleton
  sequence"), and `transform::numbers::arithmetic` makes it NaN, where XPath 1.0 takes the
  first node.

Fix (attached patch): convert a node by its string value, and every other numeric type by
its lexical form, through one helper that ignores XML white space; `number()` and each
arithmetic operand take a sequence's first item, and an empty one is NaN. This follows
XPath 1.0 (and XPath 2.0's backwards-compatible mode), which is what a `version="1.0"`
stylesheet asks for.
