# Upstream draft: complete built-in text-only-copy rules in every mode

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust), with [the patch](xrust-builtins.patch), after the pattern node-type patch.

Reproduced on the carried 2.3.0-ikigai.2 engine on 2026-11-11. With
`<doc a="A"><v>2</v><v>10</v><!--C--><?pi P?></doc>`:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:apply-templates select="doc/@* | doc/node()"/></out></xsl:template>
</xsl:stylesheet>
```

Expected `<out>A210</out>`; actual an error naming the comment as having no matching
template. Selecting only `doc/@*` silently gives `<out/>`, losing `A`.
Selecting `doc` in a named mode also errors without a matching explicit rule.
[XSLT 1.0 section 5.8](https://www.w3.org/TR/1999/REC-xslt-19991116#built-in-rule)
defines rules for every node kind, with recursion retaining the selected mode.

Create built-ins by node kind: document/element recurse, text/attribute create text,
comment/PI/namespace return nothing. Built-ins have no priority and are eligible in
every mode; clone their recursive apply-templates instruction with the caller's mode.
User rules keep their existing priority and mode filtering.

`xslt_builtin_rules_cover_each_node_kind_and_preserve_mode` covers default and named
modes, attribute text, ignored comments/PIs, and recursive mode propagation. The root
public-path audit reproduced all three failures before the fix.
