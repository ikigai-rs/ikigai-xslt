# Upstream draft: prefixed attribute values are lowercased by the XML parser

A ready-to-file issue for [xrust](https://gitlab.gnome.org/World/Rust/markup-rs/xrust) (issues
and merge requests go to GitLab; the GitHub mirror is read-only), with its patch,
[`xrust-attribute-case.patch`](xrust-attribute-case.patch). **Nothing here has been filed**:
filing is Brian's call (ledger #3342). ikigai-xslt carries the same fix in `crates/xrust`
(`ikigai-xrust` 2.3.0-ikigai.1) until a release of xrust includes it.

Where upstream stands, as checked on 2026-11-09: gitlab.gnome.org answered HTTP 406 to
scripted requests, so its `main` was not read. The GitHub mirror's `main`
(`ballsteve/xrust`, last pushed 2026-07-28) is still at 2.2.0, before this defect: there
the match is on `value.as_str()`, and `xml:space` accepts `Default` and `Preserve`. The
lowercasing arrived in 2.3.0. Check GitLab's `main` before filing.

The patch applies with `git apply` to the xrust 2.3.0 source as published on crates.io. Its
test fails there and passes with the fix, and the rest of `tests/parser.rs` still passes
(checked 2026-11-09).

---

**Title:** XML parser lowercases the value of every prefixed attribute, and of `xml:id`

In 2.3.0, parsing XML changes the case of every prefixed attribute value. So
`rdf:about="https://Example.org/My-Post"` reads back as `https://example.org/my-post`. The
same happens to `rdf:resource`, `rdf:datatype`, `dc:title`, `xml:id`, `xml:base` and
`xml:lang`, and to the variable names in an attribute value template on a prefixed
attribute in a stylesheet (`rdf:about="{$camelVar}"` then fails to find `$camelvar`). No
error is raised. Unprefixed attributes keep their case. XML 1.0 normalizes an attribute
value's whitespace, never its case, so for RDF/XML this turns any IRI with a capital letter
into a different IRI.

Reproduction (`xml::parse` into a `smite::RNode`, then `to_xml()`):

```xml
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="urn:I:One" plain="urn:I:One"/></rdf:RDF>
```

- Expected: `rdf:about='urn:I:One' plain='urn:I:One'`
- Actual: `rdf:about='urn:i:one' plain='urn:I:One'`

Cause: in `src/parser/xml/attribute.rs` (line 233 in 2.3.0), the loop over the parsed
attributes matches on `(prefix, local_part, value.to_lowercase())` so that `xml:space` can be
compared without regard to case. The `(Some("xml"), "id", val)` and `(Some(p), lp, v)` arms
then build the attribute from that lowercased binding. The unprefixed arm uses the original
`value`, which is why only prefixed attributes are affected.

Fix (attached patch): match on `value.as_str()`, and compare in lowercase only in the two
`xml:space` guards. `xml:space` behaves exactly as in 2.3.0 (any case of `default` or
`preserve` is accepted and stored in lowercase, anything else is a validation error), and
every other value is stored as written. The patch adds a test to `tests/parser.rs`.
