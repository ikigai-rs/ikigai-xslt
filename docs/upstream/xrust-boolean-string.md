# Upstream draft: a boolean's string value is empty

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-boolean-string.patch`](xrust-boolean-string.patch). **Nothing here has been filed**:
filing is Brian's call (ledger #2520, #3343). ikigai-xslt carries the same fix in
`crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one's test, added to `tests/smite-xslt.rs`, fails on xrust 2.3.0 (`<out></out>`) and passes
with the fix (checked 2026-11-09). Check GitLab's `main` before filing.

---

**Title:** `string()` of a boolean, and `xsl:value-of` of a comparison, give the empty string

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:value-of select="1 &gt; 0"/></out></xsl:template>
</xsl:stylesheet>
```

- Expected (XPath 1.0 §4.2, `string()` of a boolean is `true` or `false`): `<out>true</out>`
- Actual (2.3.0): `<out></out>`, with no error. `false()`, `not(...)`, `boolean(...)`,
  comparisons, `string(true())` and `concat('[', true(), ']')` (`[]`) are all affected.
  `xsl:if` is not: the effective boolean value is right, only the string is lost.

Cause: `impl Display for ValueData` in `src/value.rs` has no `Boolean` arm, so a boolean
falls through to `_ => ""`. Every string conversion (`Item::to_string`, so `xsl:value-of`,
`string()`, `concat()`, attribute value templates) goes through it.

Fix (attached patch): add `ValueData::Boolean(b) => b.to_string()` before the fallback.
