# Upstream draft: `number()` and arithmetic on a boolean give NaN

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-boolean-number.patch`](xrust-boolean-number.patch). **Nothing here has been filed**:
filing is Brian's call (ledger #2668, #3343). ikigai-xslt carries the same fix in
`crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one's test, added to `tests/smite-xslt.rs`, fails on xrust 2.3.0 (`<out>NaN</out>`) and
passes with the fix (checked 2026-11-09). Check GitLab's `main` before filing.

---

**Title:** `number(true())` and `true() + 1` are NaN

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:value-of select="number(true())"/></out></xsl:template>
</xsl:stylesheet>
```

- Expected (XPath 1.0 §4.4: true converts to 1, false to 0): `<out>1</out>`
- Actual (2.3.0): `<out>NaN</out>`, with no error. `number(false())`, `true() + 1` and
  `number(1 > 0) * 5` are NaN too, and `substring('yes', 1, number(1 > 0) * 3)` fails
  with "invalid digit found in string", since the length becomes NaN.

Cause: `Value::to_double` in `src/value.rs` has no `Boolean` arm and falls through to
`f64::NAN`. `number()` and arithmetic convert their operands with it.

Fix (attached patch): convert `true` to 1.0 and `false` to 0.0 in `Value::to_double`.
