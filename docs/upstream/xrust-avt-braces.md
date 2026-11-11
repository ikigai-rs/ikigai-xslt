# Upstream draft: escaped and quoted braces in attribute value templates

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust), with [the patch](xrust-avt-braces.patch), after the existing series.

Reproduced against the carried 2.3.0-ikigai.2 engine on 2026-11-11:

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:template match="/"><out a="{{x}}" b="{concat('}', '{')}"/></xsl:template>
</xsl:stylesheet>
```

With `<doc/>`, expected `<out a='{x}' b='}{'/>`; actual compilation error.
The old parser tries to parse the second opening brace as XPath, and finds the
first closing brace even inside an XPath string. A closing pair by itself is also
left doubled rather than reduced to one literal brace.
[XSLT 1.0 section 7.6.2](https://www.w3.org/TR/1999/REC-xslt-19991116#attribute-value-templates)
defines doubled braces and quotes inside the expression.

Scan the AVT once, collapse doubled braces only in literal text, and find an expression's
closing brace outside strings. Delegate each whole expression to the existing XPath
parser, preserving namespace resolution and rejecting trailing garbage.
`xslt_avt_doubled_braces_and_braces_in_xpath_strings` covers both escapes, a literal
brace around an expression, quoted braces and malformed inputs.
