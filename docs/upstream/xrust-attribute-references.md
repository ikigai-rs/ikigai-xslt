# Upstream draft: a whitespace character reference in an attribute becomes a space

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-attribute-references.patch`](xrust-attribute-references.patch). **Nothing here has
been filed**: filing is Brian's call (ledger #2520, #3343). ikigai-xslt carries the same fix
in `crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one changes `src/parser/xml/attribute.rs` after
[`xrust-attribute-case.patch`](xrust-attribute-case.patch) and applies on top of it. Its test,
added to `tests/parser.rs`, fails on xrust 2.3.0 (`a b` for `a&#10;b`) and passes with the
fix, and the rest of `tests/parser.rs` still passes (checked 2026-11-09). The W3C XML
conformance suites (`test-conformance-xml`) were not run: their data is not in the published
crate. Check GitLab's `main` before filing, and run those suites there.

---

**Title:** Attribute values: character references to tab, LF and CR become spaces, and
spaces are collapsed

```xml
<doc v="a&#10;b" w="a  b "/>
```

- Expected (XML 1.0 §3.3.3: a character reference appends the referenced character; only
  literal white space becomes a space; collapsing and trimming apply only to attributes
  declared other than CDATA): `v` is `a`, LF, `b`; `w` is `a  b ` unchanged.
- Actual (2.3.0): `v` is `a b`, `w` is `a b`, with no error. `&#xA;`, `&#x0a;`, `&#00010;`,
  `&#9;` and `&#13;` are affected the same way. In a stylesheet this makes
  `select="concat(., '&#10;')"` end with a space instead of a line feed, so no XSLT
  stylesheet can write a newline from an attribute.

Cause: `attribute_value` in `src/parser/xml/attribute.rs` collects literal text, entity
expansions and character references into one vector, then normalizes the concatenation:
`.replace(['\n', '\r', '\t', '\n'], " ")` turns referenced characters into spaces too, and
`.split_whitespace().collect::<Vec<_>>().join(" ")` collapses and trims every value.

Fix (attached patch): tag each piece with whether it came from a character reference;
replace white space with spaces in the literal and entity pieces only (counting a literal CR
LF as one line end), keep a character reference's character, and drop the collapse. A
non-CDATA attribute's normalization is left to whatever validates against the DTD; the
IDREFS handling in `src/parser/xml/element.rs` already splits on white space itself.
