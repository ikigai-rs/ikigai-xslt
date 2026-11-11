# Upstream draft: child-axis name patterns match attributes

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust), with [the patch](xrust-pattern-node-type.patch), after the existing series.

Reproduced on the carried 2.3.0-ikigai.2 engine, 2026-11-11. Source: `<doc v="A"/>`.

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><xsl:apply-templates select="doc/@v"/></xsl:template>
  <xsl:template match="v">WRONG</xsl:template>
  <xsl:template match="@*">attribute</xsl:template>
</xsl:stylesheet>
```

Expected `attribute`; actual `WRONG`. A child-axis name test has element as its
principal node type ([XSLT 1.0 section 5.2](https://www.w3.org/TR/1999/REC-xslt-19991116#patterns)).
The pattern evaluator's SelfAxis arm tests only the name. Require an element for
NameTest, while preserving KindTest matching for text, comments and processing instructions.

`xslt_element_name_patterns_do_not_match_attributes` covers named and wildcard
patterns, with and without explicit child axes. The public-path reproduction
`element_name_patterns_do_not_match_attributes` failed before the fix.
