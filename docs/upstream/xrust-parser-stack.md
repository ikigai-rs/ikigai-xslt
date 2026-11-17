# Upstream draft: the XPath parser needs ~150 KiB of stack a bracket, and has no depth bound

Nothing has been filed. Intended for [xrust on GitLab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust),
with [the patch](xrust-parser-stack.patch), after the existing series.

Reproduced against the carried 2.3.0-ikigai.2 engine on 2026-11-17 (ledger #3702), on Linux
x86_64, rustc 1.99, by parsing one expression on a thread of a given stack and halving the
interval to the smallest that runs:

| expression | stack, release |
| --- | ---: |
| `1` | 171 KiB |
| `(1)` | 323 KiB |
| `((1))` | 475 KiB |
| `(((1)))` | 627 KiB |
| `substring(concat('0', 1), number(string-length(concat('0', 1))) - 1, 2)` | 775 KiB |

About 150 KiB a level of `(`, `[` or `{`, in a release build (~250 KiB in debug), so an
ordinary date-formatting expression, compiled with the rest of its stylesheet, overflowed
the 1 MiB stack a `wasm32` module gets by default (`memory access out of bounds`). There is
no bound on the depth: 100,000 parentheses abort the process, and a stack overflow cannot be
caught.

The cost is fixed per level, not per byte. Every combinator takes and returns
`ParseInput = (&str, ParserState<N>)` by value, and `ParserState` is 608 bytes (the DTD alone,
six `HashMap`s, is 384), so each of the ~20 grammar rules the parser descends through per
bracket keeps several copies of it in its frame. Cloning it also deep-copies the DTD's tables.

The patch:

- makes `ParserState` one `Rc` to its fields (now `ParserStateFields`), reached through
  `Deref` and copied on the first write after a clone by `DerefMut` (`Rc::make_mut`). Call
  sites are unchanged but for one loop in `parser/xml/dtd/mod.rs` that now borrows the DTD
  once. `ParserState` is 8 bytes; a bracket costs ~20 KiB in release, ~35 KiB in debug, and
  an element of XML ~4 KiB in release instead of ~18;
- adds `parser::xpath::MAX_NESTING` (64) and a scan without recursion that refuses deeper
  brackets with `ErrorKind::LimitExceeded` before `xpath::parse` or `Pattern::try_from`
  parses anything. String literals are skipped (a doubled quote included), and so are
  comments, which nest.

`tests/xpath_nesting.rs` parses 64 levels of parentheses, predicates, function calls and
pattern predicates in a child process on a 2 MiB thread (8 MiB in debug), and refuses 65 and
100,000. On stock 2.3.0 the first test aborts its child, and the others do not compile (no
`MAX_NESTING`).

The public API adds `MAX_NESTING` and `ParserStateFields`. A caller that built a
`ParserState` with a struct literal cannot have: its fields were private.
