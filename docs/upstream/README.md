# Upstream drafts for xrust

Issue text and a patch for each xrust defect `ikigai-xrust` (`crates/xrust`) fixes. **None has
been filed**: filing is Brian's call. Issues and merge requests go to
[GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust); the GitHub mirror is
read-only. Check GitLab's `main` before filing each one.

The patches are a series. Applied with `git apply`, in this order, to the xrust 2.3.0 source
as published on crates.io, they give exactly the `src/` this repository carries (checked
2026-11-09); each adds its test to `tests/smite-xslt.rs` or `tests/parser.rs`, and the later
`tests/smite-xslt.rs` tests use the helper the second patch adds. Each test fails on stock
2.3.0 and passes with its fix.

| # | issue | patch | ledger |
| --- | --- | --- | --- |
| 1 | [prefixed attribute values are lowercased](xrust-attribute-case.md) | [patch](xrust-attribute-case.patch) | #2519, #3342 |
| 2 | [whitespace-only `xsl:text` is dropped](xrust-xsl-text-whitespace.md) | [patch](xrust-xsl-text-whitespace.patch) | #1693, #1715 |
| 3 | [a boolean's string value is empty](xrust-boolean-string.md) | [patch](xrust-boolean-string.patch) | #2520 |
| 4 | [`number()` and arithmetic on a boolean are NaN](xrust-boolean-number.md) | [patch](xrust-boolean-number.patch) | #2668 |
| 5 | [a comparison with a node on its left is reversed](xrust-node-comparison.md) | [patch](xrust-node-comparison.patch) | #2668 |
| 6 | [an attribute's whitespace character reference becomes a space](xrust-attribute-references.md) | [patch](xrust-attribute-references.patch) | #2520 |
| 7 | [`number()` and arithmetic on a node, or `string-length()`, are NaN](xrust-numeric-nodes.md) | [patch](xrust-numeric-nodes.patch) | #2487 |
| 8 | [an integer literal past `i64` is NaN](xrust-large-integer-literal.md) | [patch](xrust-large-integer-literal.patch) | #2668 |
| 9 | [a literal attribute in a namespace panics in serialization](xrust-literal-attribute-namespace.md) | [patch](xrust-literal-attribute-namespace.patch) | #2520 |

Patch 6 changes the same file as patch 1 and applies only on top of it; the others touch
different code and can be filed in any order, though a test in patches 3 to 9 needs patch
2's `transform_str` helper in `tests/smite-xslt.rs` (or a copy of it).

Not drafted, from [`../upstream-xrust.md`](../upstream-xrust.md) and the ledger (ledger
#3343's hand-back says why):

- the process-wide name interner lock (#2826, #2852): it is `qualname`'s, not xrust's, and
  `QName` is in xrust's public API;
- import and include modules (#192): the empty document came from ikigai-xslt's own fetcher,
  which since 0.6.0 resolves modules itself; xrust's `from_document` took what it was given;
- match-pattern predicates and template priority (cases 2 and 3), `position()`, `last()` and
  `[last()]` (cases 4 and 5), and a leading `//` away from the root (case 6): each needs a
  change to how patterns or the focus are evaluated, a design call for upstream;
- `xsl:value-of` over several nodes (case 7), XPath 1.0 relational comparison of strings, and
  unary `-`/`+`: XPath 1.0 and 2.0 disagree on the first two, which is a design call; unary
  signs were not reached.
