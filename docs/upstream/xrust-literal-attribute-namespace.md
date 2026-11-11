# Upstream draft: a literal attribute in a namespace panics in serialization

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-literal-attribute-namespace.patch`](xrust-literal-attribute-namespace.patch).
**Nothing here has been filed**: filing is Brian's call (ledger #2520, #3343). ikigai-xslt
carries the same fix in `crates/xrust` (`ikigai-xrust`) until a release of xrust includes it.

The patches in this directory are a series: see [README.md](README.md) for the order. This
one's test, added to `tests/smite-xslt.rs`, panics on xrust 2.3.0 (`unable to find
namespace`) and passes with the fix (checked 2026-11-09). Check GitLab's `main` before
filing.

---

**Title:** `<out><b rdf:datatype="x"/></out>` panics: "unable to find namespace"

```xml
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <xsl:template match="/"><out><b rdf:datatype="x">1</b></out></xsl:template>
</xsl:stylesheet>
```

- Expected (XSLT 1.0 §7.1.1: the result carries the namespace nodes a literal result
  element has in the stylesheet): `<out><b xmlns:rdf="…" rdf:datatype="x">1</b></out>`
- Actual (2.3.0): serializing the result panics with `unable to find namespace`. Declaring
  `xmlns:rdf` on `b` itself does not help; wrapping in `<rdf:RDF>` does.

Cause: compiling a literal result element in `src/xslt.rs` emits a
`Transform::NamespaceDeclaration` only for the element's own name, then copies its
attributes. The result tree then holds an attribute whose namespace no element declares,
and the serializer (`src/trees/smite.rs`) panics resolving its prefix.

Fix (attached patch): emit a declaration, with its in-scope prefix, for each distinct
namespace a literal attribute's name is in (other than XSLT's, the element's own, and
`xml:`). This is narrower than XSLT 1.0, which copies every in-scope namespace except the
excluded ones, and does not touch the serializer, whose panic on an unresolvable name would
be better as an error but needs an API change.
