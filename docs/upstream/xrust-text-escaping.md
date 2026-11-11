# Upstream draft: xsl:text escapes twice and ignores its output hint

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust), with [the patch](xrust-text-escaping.patch), after the existing patch series.

Reproduced against the carried 2.3.0-ikigai.2 engine on 2026-11-11, before this fix:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out><xsl:text>&lt;b/&gt;&amp;</xsl:text></out></xsl:template>
</xsl:stylesheet>
```

With `<doc/>`, expected `<out>&lt;b/&gt;&amp;</out>`; actual
`<out>&amp;lt;b/&amp;gt;&amp;amp;</out>`. With `disable-output-escaping="yes"`,
expected `<out><b/>&</out>` but actual `<out>&lt;b/&gt;&amp;</out>`.
[XSLT 1.0 sections 7.2 and 16.4](https://www.w3.org/TR/1999/REC-xslt-19991116#disable-output-escaping)
require characters in the result text node and an output hint, rather than pre-escaped characters.

Compile xsl:text to LiteralText with a literal character value and the requested OutputSpec,
just as value-of creates text nodes. Reject invalid hint values for every output method.
The regression `xslt_text_escapes_once_and_honors_disable_output_escaping` fails before
and passes after the fix; it covers the default and explicit yes/no.
