# Upstream draft: every path step deep-copies the stylesheet's named templates, variables and keys

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust),
with [the patch](xrust-context-share.patch), after the existing series. Search the tracker
first.

---

Title: Every path step deep-clones the stylesheet's named templates (Context::callables is held by value)

**Summary**

`Context` holds `callables: HashMap<QName, Callable<N>>` by value, and each `Callable` owns its
body as a `Transform<N>` tree. `Context` is cloned on every step of a path: `compose`
(`src/transform/navigate.rs`) clones the context once and then once per step through
`ContextBuilder::from(&Context)` (`src/transform/context.rs`), which is
`ContextBuilder(c.clone())`. So evaluating any multi-step path deep-copies, and then drops, the
bodies of every named template and function in the stylesheet. The same applies to `pre_vars`
and `keys`. Match templates are not affected: they are already `Vec<Rc<Template<N>>>`.

Checked in 2.3.0 and 2.4.1 (the source on docs.rs).

**Measurement**

`benches/bench_context.rs` (in the patch) compiles one stylesheet and runs it over a document
of 200 rows. A match template evaluates four multi-step paths a row (`a/b/c`, `a/b/d`, `e/f`,
`count(a/b/*)`). Beside it the stylesheet declares N named templates, N stylesheet variables
or N keys, none of which is ever called, read or used. Release build (criterion), Apple M5 Max,
rustc 1.100, on 2.3.0 with the earlier patches in this series (none of which touches
`Context`), median time per row:

| declarations | N = 0 | N = 10 | N = 40 |
| --- | ---: | ---: | ---: |
| named templates, before | 18 µs | 256 µs | 1,137 µs |
| named templates, after | 17 µs | 17 µs | 17 µs |
| keys, before | 18 µs | 106 µs | 491 µs |
| keys, after | 18 µs | 18 µs | 18 µs |
| stylesheet variables, before | 18 µs | 116 µs | 432 µs |
| stylesheet variables, after | 18 µs | 56 µs | 184 µs |

Each row costs about 28 µs more for every named template the stylesheet declares (each here is a
small `xsl:param`, `xsl:if` and `xsl:value-of`), so a stylesheet gets slower as it gains named
templates even when they are never called. In a server-side HTML renderer with one ~200 KB
stylesheet (37 named templates, 125 match templates), a profile showed about 91% of the
transform thread in `Transform<N>` clone and drop, and a cold render cost 32 to 38 ms a row.

Stylesheet variables still grow after the fix, by about 4 µs a row per variable. Their
evaluated values live in `vars`, a `RefCell<HashMap<String, Vec<Sequence<N>>>>` that is cloned
with the context too, and the patch leaves it alone on purpose: its scoping relies on the
clone, so sharing it needs a different design (a persistent map, or a chain of scopes).

**Suggested fix (the patch)**

Share the parts of `Context` that do not change after the stylesheet is compiled:

- `callables: Rc<HashMap<QName, Callable<N>>>`, with `Rc::make_mut` where `callable_push` and
  `ContextBuilder::callable` insert, so neither changes its signature;
- the same for `pre_vars` (`Rc<Vec<…>>`, in `pre_var_push`) and `keys` (in `declare_key`).

`Context` still derives `Clone`, and a clone becomes three reference-count increments for these
fields. The fields are `pub(crate)`, so the public API does not change.
`callable_get` still returns an owned `Callable` (a deep copy of its body), once a call rather
than once a step; returning `Rc<Callable<N>>` would make that cheap too, but changes its
signature, so the patch does not.

`xslt_named_template_from_nested_path_sees_its_scope` (in `tests/smite-xslt.rs`) pins what must
not change: a named template called from inside a `for-each` over a path sees its parameters,
the caller's focus, stylesheet variables, keys and the row's local variable, and calls another
named template with a parameter. It passes before and after; the bench is the measurement.

We are glad to test a branch against our stylesheet and report numbers, or to send a merge
request if you would take one.

---

Notes (not for the issue):

- Measured 2026-11-22 for ledger #4053 on the carried 2.3.0-ikigai.2 engine; the fix is in
  ikigai-xrust 2.3.0-ikigai.3.
- The hub's first draft also mentioned 2.2.0's `unattached`/`detach` scan in `trees/smite.rs`
  as a separate quadratic. Not measured here, so left out; file it on its own with numbers.
