# Upstream draft: whitespace-only `xsl:text` is dropped

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-xsl-text-whitespace.patch`](xrust-xsl-text-whitespace.patch). **Nothing here has been
filed**: filing is Brian's call (ledger #1715). ikigai-xslt carries the same fix in
`crates/xrust` (`ikigai-xrust`, ledger #3343) until a release of xrust includes it.

The patch applies with `git apply` to the xrust 2.3.0 source as published on crates.io. Its
test, added to `tests/smite-xslt.rs`, fails there (`<out>MetaSource</out>`) and passes with
the fix, and the rest of `tests/smite-xslt.rs` still passes (checked 2026-11-09). Check
GitLab's `main` before filing.

---

**Title:** Whitespace-only `xsl:text` is removed from the stylesheet

An `xsl:text` whose content is only whitespace produces nothing:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out>Meta<xsl:text> </xsl:text>Source</out></xsl:template>
</xsl:stylesheet>
```

- Expected (XSLT 1.0 §3.4 and §7.2: whitespace in `xsl:text` is never stripped):
  `<out>Meta Source</out>`
- Actual (2.3.0): `<out>MetaSource</out>`, with no error. Two spaces, a tab and newline,
  and `&#x20;` are lost the same way. `<xsl:text>Meta </xsl:text>` keeps its space.

Cause: `xslt::from_document` calls `transform::whitespace::strip_whitespace` with a wildcard
strip rule and a preserve rule for `xsl:text`. In `strip_whitespace_node`, the loop over the
preserve rules assigns each match's score to `ss`, the strip score, instead of `ps`, the
preserve score (only its `KindTest` arm sets `ps`). So `ps` stays at -1, the children of
`xsl:text` are visited with `keep = false`, and the whitespace-only text node is removed
before the instruction is compiled. The same applies to `xsl:preserve-space` rules for a
source document.

Fix (attached patch): assign the preserve rules' scores to `ps`. Indentation between
stylesheet elements is still stripped; the patch's test checks both.
