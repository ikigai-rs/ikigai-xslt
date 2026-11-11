# Upstream draft: xsl:text escapes characters twice

Not filed. Patch: [xrust-text-escaping.patch](xrust-text-escaping.patch), applied after
patch 8 in [README.md](README.md). Reproduced on the carried 2.3.0-ikigai.2 fork
by `xslt_text_escapes_only_during_serialization` in `tests/smite-xslt.rs` and the
public `text_is_escaped_once` audit test before changing the engine.

For source `<doc/>` and a root template containing
`<out><xsl:text>&lt;&amp;</xsl:text></out>`, expected XML is
`<out>&lt;&amp;</out>`; actual XML was `<out>&amp;lt;&amp;amp;</out>`.
Explicit `disable-output-escaping="no"` has the same defect.

[XSLT 1.0 §7.2](https://www.w3.org/TR/1999/REC-xslt-19991116#section-Creating-Text)
creates text containing characters. The compiler instead pre-escaped those characters,
and the serializer escaped the resulting ampersands again. Compile xsl:text to a
LiteralText with its OutputSpec, preserving the characters until serialization.
This also preserves the text method's string value and honors explicit disabling
of output escaping. No public API change. Check GitLab main before submission.
