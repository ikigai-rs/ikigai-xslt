# Upstream draft: a comparison with a node on its left is evaluated backwards

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-node-comparison.patch`](xrust-node-comparison.patch). **Nothing here has been filed**:
filing is Brian's call (ledger #2668, #3343). ikigai-xslt carries the same fix in
`crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one's test, added to `tests/smite-xslt.rs`, fails on xrust 2.3.0 (`/r/v > 2` is false) and
passes with the fix (checked 2026-11-09). Check GitLab's `main` before filing.

---

**Title:** `/r/v > 2` is false when `v` is 3: node-to-value comparisons swap their operands

Source `<r><v>3</v></r>`:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:choose><xsl:when test="/r/v &gt; 2">yes</xsl:when><xsl:otherwise>no</xsl:otherwise></xsl:choose></out></xsl:template>
</xsl:stylesheet>
```

- Expected (XPath 1.0 §3.4): `<out>yes</out>`
- Actual (2.3.0): `<out>no</out>`, with no error. `/r/v < 2`, `>=` and `<=` are reversed the
  same way; `2 < /r/v` (value on the left) and `=` are right.

Cause: `Item::compare` in `src/item.rs` handles a node on the left by calling
`other.compare(string(self), op)`: the operands change places but `op` does not, so
`3 > 2` is evaluated as `2 > 3`.

Fix (attached patch): mirror the operator when the operands are exchanged (`<` and `>`,
`<=` and `>=`, `<<` and `>>`), with a private helper, so no public API changes.
