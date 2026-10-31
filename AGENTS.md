# AGENTS.md

`ikigai-xslt` is XSLT transformation as an ikigai resource (`urn:xslt:transform`), in pure Rust
and wasm-ready.

- Read [ACCESSIBILITY.md](ACCESSIBILITY.md) before any change to output people read: messages,
  pages, terminal text.
- Run fmt, clippy (`-D warnings`) and the tests before opening a pull request; CI runs the shared
  Rust workflow, with a wasm lint of the `module` feature.
- Security reports go through
  [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-xslt/security/advisories/new),
  never a public issue.
- Write American spelling (behavior, color, labeled).
