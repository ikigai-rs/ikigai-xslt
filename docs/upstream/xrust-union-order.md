# Upstream draft: union concatenates operands without sorting or deduplicating

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust), with [the patch](xrust-union-order.patch), after the existing series.

Reproduced on the carried engine on 2026-11-11. With `<doc><v>2</v><v>10</v></doc>`:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:for-each select="doc/v[2] | doc/v[1]"><xsl:value-of select="."/></xsl:for-each></out></xsl:template>
</xsl:stylesheet>
```

Expected `<out>210</out>`; actual `<out>102</out>`. This also causes a compatibility
adapter taking the first union node to choose the wrong node. The public-path test
`value_of_uses_the_first_node_in_document_order` failed with `10` instead of `2`.

XPath 1.0 [section 3.3](https://www.w3.org/TR/1999/REC-xpath-19991116/#node-sets)
defines union of node sets; XSLT for-each processes them in document order. XPath 3.1
also requires union results in document order without duplicates.

Use the same document-order comparison and node-identity deduplication as navigation.
`xslt_union_is_in_document_order_without_duplicate_nodes` covers reversed operands,
repeated selections and overlapping selections. Namespace bindings and split match-pattern
unions are untouched; their existing regressions remain in the root suite.
