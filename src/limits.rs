//! Bounds on caller-supplied XML, checked before xrust sees a byte (ledger #916).
//!
//! xrust parses, compiles, evaluates, serializes and drops by **recursion**, so the shape of
//! either input is a claim on the stack of whatever thread runs it, and running out is not an
//! error a caller gets back: Rust aborts the **whole process** on a stack overflow, on any
//! thread. Measured on a 2 MiB thread (a tokio worker's stack) through `urn:xslt:transform`
//! (xrust 2.2.0, `tests/xml_depth.rs`): 24 nested source elements abort a debug build and 114
//! a release one, a stylesheet nesting 22 (debug) or 112 (release) literal result elements
//! does the same, and an XPath nesting 7 (debug) or 13 (release) parentheses or predicates —
//! about 40 bytes of stylesheet — aborts the process. Every host that binds the endpoint,
//! including one whose anonymous door offers it, was one request from going down.
//!
//! Two layers, because neither is enough alone:
//!
//! 1. **[`check_source`] and [`check_stylesheet`] refuse, never truncate.** A document whose
//!    elements nest deeper than [`MAX_XML_DEPTH`], a stylesheet whose attribute values (its
//!    XPath expressions and attribute value templates) nest brackets deeper than
//!    [`MAX_EXPR_DEPTH`], and a document type declaration that could build markup the scan
//!    cannot see, are refused with a typed `InvalidArgument` naming the argument and the
//!    bound — before anything is parsed. The scan is one non-recursive pass.
//! 2. **[`on_xslt_stack`] runs the whole transform on a thread sized for those bounds**, so
//!    anything they admit runs in a debug and a release build alike. Nesting in the text is
//!    not the only recursion: a template that calls itself nests the RESULT once per call,
//!    and xrust allows [`XRUST_MAX_DEPTH`] calls whatever the text says, so the result can
//!    be [`XRUST_MAX_DEPTH`] times deeper than the stylesheet. No lexical bound refuses that
//!    without refusing ordinary recursive templates; the thread is where that budget lives.
//!    A transform through a stylesheet runs on THAT stylesheet's thread, which compiled it
//!    once and keeps it (ledger #453, #2826); see [`MAX_STYLESHEET_THREADS`].
//!
//! ## Declarations (`<!`)
//!
//! Comments, CDATA sections and processing instructions are skipped exactly: markup inside
//! them is not nesting. A document type declaration is read through its internal subset,
//! because xrust EXPANDS general entities and parses their replacement text as content, so an
//! entity whose value holds `<a><a>…` nests where the tag scan cannot see it — and one whose
//! value references another entity multiplies (eight levels of that is xrust's own limit, and
//! ten references a level is 10^8 copies). So an entity declaration is refused when its value
//! holds `<`, `&` or `%` (markup, a reference, or a character reference that becomes markup
//! once expanded), when it is a parameter entity (`<!ENTITY %`, which can build declarations),
//! or when it is external (`SYSTEM`/`PUBLIC`). A parameter-entity reference in the subset is
//! refused too. What remains is the declaration RDF/XML actually uses — a name for a
//! namespace IRI, `<!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">` — and those
//! expand to text. Their total expansion is still bounded, by [`MAX_ENTITY_EXPANSION`]: a
//! short document referencing a long value many times is a memory bomb even when it is flat.
//! Element, attribute-list and notation declarations are skipped (quotes honored). An external
//! DTD is never fetched: this crate gives xrust no resolver.
//!
//! ⚠ **On wasm there is no thread**: [`on_xslt_stack`] runs inline there, so only the first
//! layer applies, on whatever stack the host gave the module.
//!
//! ## Prefixed attribute values (ledger #2519, #3342)
//!
//! xrust 2.3.0 lowercases every prefixed attribute value while parsing XML, so 0.5.0 through
//! 0.7.0 refused one whose case would change. Since 0.8.0 this crate builds on `ikigai-xrust`
//! (`crates/xrust`), xrust 2.3.0 with that defect fixed (among others): those values come through as
//! written, and nothing is refused for their case.
//!
//! ## Attribute whitespace references (ledger #2520)
//!
//! xrust 2.3.0 normalizes a referenced tab, LF or CR in an attribute to a space, and 0.5.0
//! through 0.8.0 refused such a reference in either input. The xrust this crate carries keeps
//! the referenced character, as XML 1.0 §3.3.3 says (ledger #3343), so nothing is refused.
//!
//! ## Time (ledger #1040)
//!
//! Neither layer bounds TIME, and some shapes inside both are super-linear in xrust: a long
//! operator chain in the compile, a template recursing to build a deep result about as the
//! cube of its depth. So [`on_xslt_stack_within`] answers the caller at a deadline —
//! [`DEFAULT_TIME_BUDGET`], 5 s, for everything this crate runs on its own — with a typed
//! `Timeout`, never a partial result. ⚠ The work is **abandoned, not cancelled**: xrust has
//! no cancellation point, so it runs to its end on its own thread, and what bounds the CPU is
//! a cap on how many may be running like that at once ([`max_overdue_transforms`]). See
//! [`on_xslt_stack_within`] and [`DEFAULT_TIME_BUDGET`] for the evidence and the trade.
//!
//! ## Panics
//!
//! xrust panics on some input a caller controls (an attribute or the document node at the top
//! of the result, an `xsl:sort` key that fails to evaluate). [`on_xslt_stack`] and
//! [`on_xslt_stack_within`] catch it on the pooled thread and answer `Endpoint` naming it.

use ikigai_core::{Error, Result};
use std::sync::OnceLock;
use std::time::Duration;

/// The deepest either input's elements may nest — **64**.
///
/// Real use is shallow: gonk's 98 KB stylesheet nests 14, the reading room's stylesheets 5
/// to 9, and RDF/XML a handful. On a 2 MiB thread xrust overflows at ~24 levels in a debug
/// build and ~250 in release; the transform runs on [`on_xslt_stack`], sized for this bound in
/// both.
pub const MAX_XML_DEPTH: usize = 64;

/// Byte budget for an assembled stylesheet: **8 MiB**. Counts the main source and
/// every imported/included source occurrence (including wrappers and comments),
/// plus template copies needed to give union alternatives separate priorities.
/// Checked before each occurrence is parsed or copied; repeated modules each count.
pub const MAX_ASSEMBLED_STYLESHEET_BYTES: usize = 8 * 1024 * 1024;

/// The deepest the brackets `(`, `[` and `{` may nest inside any one attribute value of a
/// stylesheet — **32**. That is where XPath lives (`select`, `test`, `match`, and the `{…}`
/// of an attribute value template), and xrust's XPath parser is the costliest recursion per
/// level measured: ~250 KiB of stack a level in a debug build, so 7 levels overflow a 2 MiB
/// thread. Real stylesheets nest two or three.
///
/// A character reference that decodes to a bracket counts as one, and a reference to a
/// declared entity counts every opening bracket of its value, closing nothing — it may count
/// more than the parser sees, never less. Only the stylesheet is held to this: a source
/// document's attribute values are data, never parsed as expressions.
pub const MAX_EXPR_DEPTH: usize = 32;

/// The most text the general entities a document declares may expand to, summed over every
/// reference — **1 MiB**. See the module docs: a flat memory bomb, not a stack one.
pub const MAX_ENTITY_EXPANSION: usize = 1 << 20;

/// xrust's own bound on evaluation depth (template and function calls): **200**. It is
/// `pub(crate)` in xrust (`transform::MAXDEPTH`), so it is restated here, and a test pins the
/// behavior: call 200 deep and xrust refuses with "exceeded evaluation depth".
pub const XRUST_MAX_DEPTH: usize = 200;

/// The shape [`scan`] reads from one document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Shape {
    /// The deepest element nesting.
    pub depth: usize,
    /// The deepest bracket nesting inside any one attribute value.
    pub expr_depth: usize,
    /// The text the document's general entity references expand to, in bytes.
    pub entity_expansion: usize,
}

/// Refuse a source document that nests deeper than [`MAX_XML_DEPTH`] or carries a
/// declaration [`scan`] refuses, naming the argument `arg`. Never truncates.
///
/// ```
/// use ikigai_xslt::limits::{check_source, MAX_XML_DEPTH};
///
/// assert!(check_source("<rdf:RDF><a b='(((((((((('/></rdf:RDF>", "src").is_ok());
/// let deep = format!("{}{}", "<a>".repeat(100), "</a>".repeat(100));
/// let refusal = check_source(&deep, "src").unwrap_err().to_string();
/// assert!(refusal.contains("`src`") && refusal.contains(&MAX_XML_DEPTH.to_string()));
/// // Markup in a comment or a CDATA section is not nesting.
/// let quoted = format!("<a><!-- {} --><![CDATA[{}]]></a>", "<b>".repeat(100), "<c>".repeat(100));
/// assert!(check_source(&quoted, "src").is_ok());
/// ```
pub fn check_source(text: &str, arg: &str) -> Result<Shape> {
    let shape = scan(text).map_err(|detail| refuse(arg, detail))?;
    if shape.depth > MAX_XML_DEPTH {
        return Err(refuse(
            arg,
            format!(
                "the document nests {} elements deep, deeper than {MAX_XML_DEPTH} \
                 (MAX_XML_DEPTH): the XSLT engine recurses once per level",
                shape.depth
            ),
        ));
    }
    Ok(shape)
}

/// [`check_source`], and also refuse a stylesheet whose attribute values nest brackets
/// deeper than [`MAX_EXPR_DEPTH`].
///
/// ```
/// use ikigai_xslt::limits::{check_stylesheet, MAX_EXPR_DEPTH};
///
/// let ok = r#"<x:stylesheet xmlns:x="http://www.w3.org/1999/XSL/Transform">
///   <x:template match="/"><x:value-of select="count(//a[b])"/></x:template></x:stylesheet>"#;
/// assert!(check_stylesheet(ok, "stylesheet").is_ok());
/// let deep = ok.replace("count(//a[b])", &format!("{}1{}", "(".repeat(40), ")".repeat(40)));
/// let refusal = check_stylesheet(&deep, "stylesheet").unwrap_err().to_string();
/// assert!(refusal.contains("`stylesheet`") && refusal.contains(&MAX_EXPR_DEPTH.to_string()));
/// ```
pub fn check_stylesheet(text: &str, arg: &str) -> Result<Shape> {
    let shape = check_source(text, arg)?;
    if shape.expr_depth > MAX_EXPR_DEPTH {
        return Err(refuse(
            arg,
            format!(
                "an expression nests brackets {} deep, deeper than {MAX_EXPR_DEPTH} \
                 (MAX_EXPR_DEPTH): the XPath parser recurses once per level",
                shape.expr_depth
            ),
        ));
    }
    Ok(shape)
}

fn refuse(arg: &str, detail: String) -> Error {
    Error::InvalidArgument {
        name: arg.to_string(),
        detail,
    }
}

/// Read a document's [`Shape`] in one pass, with no recursion and no allocation beyond the
/// declared entities' summaries — or refuse a declaration that could build what the pass
/// cannot see.
/// The pass is deliberately not a parser: it may count
/// MORE nesting than xrust would (a malformed document xrust refuses anyway), never less.
pub fn scan(text: &str) -> std::result::Result<Shape, String> {
    let b = text.as_bytes();
    let mut shape = Shape::default();
    // The declared general entities, summarized without expanding or copying their values.
    let mut entities: Vec<Entity<'_>> = Vec::new();
    let (mut depth, mut at) = (0usize, 0usize);
    while at < b.len() {
        match b[at] {
            b'<' => {}
            b'&' => {
                // A reference in content: only a declared entity expands to anything.
                at = expand(b, at, &entities, &mut shape)?.0;
                continue;
            }
            _ => {
                at += 1;
                continue;
            }
        }
        let rest = &b[at..];
        if rest.starts_with(b"<!--") {
            at = skip_past(b, at + 4, b"-->");
        } else if rest.starts_with(b"<![CDATA[") {
            at = skip_past(b, at + 9, b"]]>");
        } else if rest.starts_with(b"<?") {
            at = skip_past(b, at + 2, b"?>");
        } else if rest.starts_with(b"<!DOCTYPE") {
            at = doctype(b, at + 9, &mut entities)?;
        } else if rest.starts_with(b"<!") {
            // Not legal in content; xrust refuses it. Skip it as a declaration.
            at = tag_end(b, at + 2).0;
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            at = tag_end(b, at + 2).0;
        } else {
            let (end, self_closed) = start_tag(b, at + 1, &entities, &mut shape)?;
            if self_closed {
                shape.depth = shape.depth.max(depth + 1);
            } else {
                depth += 1;
                shape.depth = shape.depth.max(depth);
            }
            at = end;
        }
    }
    Ok(shape)
}

/// The index just past the first `close` at or after `from`, or the end of the text.
fn skip_past(b: &[u8], from: usize, close: &[u8]) -> usize {
    b[from.min(b.len())..]
        .windows(close.len())
        .position(|w| w == close)
        .map_or(b.len(), |i| from + i + close.len())
}

/// The index just past the `>` that ends a tag whose body starts at `from` (quotes honored),
/// and whether the byte before it is `/`.
fn tag_end(b: &[u8], from: usize) -> (usize, bool) {
    let mut quote: Option<u8> = None;
    let mut at = from;
    while at < b.len() {
        match (quote, b[at]) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'>') => return (at + 1, at > from && b[at - 1] == b'/'),
            (None, _) => {}
        }
        at += 1;
    }
    (b.len(), false)
}

/// Only summaries are kept: the bounds prohibit entities containing further references.
struct Entity<'a> {
    name: &'a [u8],
    len: usize,
    opens: usize,
}

/// Read a start tag whose body starts at `from`, measuring the bracket nesting of each
/// attribute value. Returns the index past its `>` and whether it closed itself.
fn start_tag(
    b: &[u8],
    from: usize,
    entities: &[Entity<'_>],
    shape: &mut Shape,
) -> std::result::Result<(usize, bool), String> {
    let mut at = from;
    while at < b.len() {
        match b[at] {
            b'>' => return Ok((at + 1, at > from && b[at - 1] == b'/')),
            q @ (b'"' | b'\'') => {
                let mut level = 0usize;
                at += 1;
                while at < b.len() && b[at] != q {
                    match b[at] {
                        b'(' | b'[' | b'{' => {
                            level += 1;
                            shape.expr_depth = shape.expr_depth.max(level);
                        }
                        b')' | b']' | b'}' => level = level.saturating_sub(1),
                        b'&' => {
                            let (end, opens, closes) = expand(b, at, entities, shape)?;
                            level += opens;
                            shape.expr_depth = shape.expr_depth.max(level);
                            level = level.saturating_sub(closes);
                            at = end;
                            continue;
                        }
                        _ => {}
                    }
                    at += 1;
                }
                at += 1;
            }
            _ => at += 1,
        }
    }
    Ok((b.len(), false))
}

/// Account for the reference starting at the `&` at `at`: a character reference that decodes
/// to a bracket is one (returned as an open or a close), a declared entity adds its length to
/// the expansion and its opening brackets as opens (closing nothing). Returns the index past
/// the reference and the opens and closes it stands for.
fn expand(
    b: &[u8],
    at: usize,
    entities: &[Entity<'_>],
    shape: &mut Shape,
) -> std::result::Result<(usize, usize, usize), String> {
    let Some(semi) = b[at..].iter().take(64).position(|&c| c == b';') else {
        return Ok((at + 1, 0, 0));
    };
    let name = &b[at + 1..at + semi];
    let end = at + semi + 1;
    if let Some(num) = name.strip_prefix(b"#") {
        let code = match num.strip_prefix(b"x") {
            Some(hex) => std::str::from_utf8(hex)
                .ok()
                .and_then(|h| u32::from_str_radix(h, 16).ok()),
            None => std::str::from_utf8(num).ok().and_then(|d| d.parse().ok()),
        };
        return Ok(match code {
            Some(0x28 | 0x5B | 0x7B) => (end, 1, 0),
            Some(0x29 | 0x5D | 0x7D) => (end, 0, 1),
            _ => (end, 0, 0),
        });
    }
    if let Some(entity) = entities.iter().find(|e| e.name == name) {
        shape.entity_expansion = shape.entity_expansion.saturating_add(entity.len);
        if shape.entity_expansion > MAX_ENTITY_EXPANSION {
            return Err(format!(
                "its entity references expand to more than {MAX_ENTITY_EXPANSION} bytes \
                 (MAX_ENTITY_EXPANSION)"
            ));
        }
        return Ok((end, entity.opens, 0));
    }
    Ok((end, 0, 0))
}

/// Read a document type declaration whose body starts at `from` (just past `<!DOCTYPE`),
/// recording each internal general entity and refusing what the module docs refuse. Returns
/// the index past its closing `>`.
fn doctype<'t>(
    b: &'t [u8],
    from: usize,
    entities: &mut Vec<Entity<'t>>,
) -> std::result::Result<usize, String> {
    // The name and any external identifier, up to the internal subset or the end.
    let mut at = from;
    let mut quote: Option<u8> = None;
    while at < b.len() {
        match (quote, b[at]) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c @ (b'"' | b'\'')) => quote = Some(c),
            (None, b'>') => return Ok(at + 1),
            (None, b'[') => break,
            (None, _) => {}
        }
        at += 1;
    }
    at += 1;
    // The internal subset.
    while at < b.len() {
        let rest = &b[at..];
        if rest[0] == b']' {
            return Ok(tag_end(b, at + 1).0);
        } else if rest[0] == b'%' {
            return Err(
                "its document type declaration references a parameter entity, which \
                 can build declarations the bound cannot read"
                    .to_string(),
            );
        } else if rest.starts_with(b"<!--") {
            at = skip_past(b, at + 4, b"-->");
        } else if rest.starts_with(b"<?") {
            at = skip_past(b, at + 2, b"?>");
        } else if rest.starts_with(b"<!ENTITY") {
            at = entity(b, at + 8, entities)?;
        } else if rest[0] == b'<' {
            at = tag_end(b, at + 1).0;
        } else {
            at += 1;
        }
    }
    Ok(b.len())
}

/// Read one `<!ENTITY` declaration whose body starts at `from`, recording it or refusing it.
fn entity<'t>(
    b: &'t [u8],
    from: usize,
    entities: &mut Vec<Entity<'t>>,
) -> std::result::Result<usize, String> {
    let mut at = from;
    let skip_space = |mut at: usize| {
        while at < b.len() && b[at].is_ascii_whitespace() {
            at += 1;
        }
        at
    };
    at = skip_space(at);
    if b.get(at) == Some(&b'%') {
        return Err(
            "it declares a parameter entity, which can build declarations the bound cannot read"
                .to_string(),
        );
    }
    let name_start = at;
    while at < b.len() && !b[at].is_ascii_whitespace() && b[at] != b'>' {
        at += 1;
    }
    let name = &b[name_start..at];
    at = skip_space(at);
    match b.get(at) {
        Some(&q @ (b'"' | b'\'')) => {
            let start = at + 1;
            let len = b[start..]
                .iter()
                .position(|&c| c == q)
                .unwrap_or(b.len() - start);
            let value = &b[start..start + len];
            if let Some(&c) = value.iter().find(|&&c| matches!(c, b'<' | b'&' | b'%')) {
                return Err(format!(
                    "the entity `{}` holds `{}`: an entity that expands to markup or to another \
                     reference can nest or multiply where the bound cannot see it",
                    String::from_utf8_lossy(name),
                    c as char
                ));
            }
            let opens = value
                .iter()
                .filter(|&&c| matches!(c, b'(' | b'[' | b'{'))
                .count();
            entities.push(Entity { name, len, opens });
            Ok(tag_end(b, start + len + 1).0)
        }
        _ => Err(format!(
            "the entity `{}` is external (or malformed): only an internal entity whose value is \
             plain text is admitted",
            String::from_utf8_lossy(name)
        )),
    }
}

/// The stack an [`on_xslt_stack`] thread is given before the per-level shares: 8 MiB.
pub const STACK_BASE: usize = 8 << 20;

/// The stack reserved per level of element nesting, per [`MAX_XML_DEPTH`] level of each of the
/// two inputs: 128 KiB in a build with `debug_assertions`, 32 KiB without. Measured
/// (`tests/xml_depth.rs`, `measure_stack`, xrust 2.2.0) at ~82 KiB a level in a debug build and
/// ~18 KiB in release — parsing dominates; evaluating, copying and serializing the same tree
/// cost no more.
pub const STACK_PER_ELEMENT: usize = if cfg!(debug_assertions) {
    128 << 10
} else {
    32 << 10
};

/// The stack reserved per level of bracket nesting in an expression, per [`MAX_EXPR_DEPTH`]
/// level: 384 KiB with `debug_assertions`, 256 KiB without. Measured at ~250 KiB a level in a
/// debug build and ~145 KiB in RELEASE — parentheses, predicates, and the `{…}` of an
/// attribute value template alike — so 13 levels overflowed a 2 MiB thread even optimized.
pub const STACK_PER_EXPR: usize = if cfg!(debug_assertions) {
    384 << 10
} else {
    256 << 10
};

/// The stack reserved per level of the RESULT tree's nesting: 24 KiB with `debug_assertions`,
/// 2 KiB without. A template that calls itself inside `n` literal elements nests the result
/// `n` levels a call, up to [`XRUST_MAX_DEPTH`] calls, so the result can nest
/// [`XRUST_MAX_DEPTH`] × [`MAX_XML_DEPTH`] deep with nothing in the text deeper than the bound.
/// Measured at ~11–14 KiB a level in a debug build (`style-recnest`) and too little to see in
/// release (a 400-level result runs in 192 KiB). Recursion through an `xsl:function` costs no
/// stack a call in either build (`style-fnrec`), so it has no term of its own.
pub const STACK_PER_RESULT_LEVEL: usize = if cfg!(debug_assertions) {
    24 << 10
} else {
    2 << 10
};

/// The stack every [`on_xslt_stack`] thread gets: enough for anything the bounds admit, in
/// this build — ~45 MiB in release and ~338 MiB with `debug_assertions`. A thread's stack is
/// reserved address space, committed only as it is touched (and kept, once touched, for as
/// long as the pooled thread lives: [`release_idle_threads`] lets them go).
///
/// ```
/// use ikigai_xslt::limits::xslt_stack_size;
/// // A doctest's own `debug_assertions` need not match the library's build, so: either size.
/// let mib = xslt_stack_size() >> 20;
/// assert!(mib == 45 || mib == 337, "{mib}");
/// ```
///
/// `debug_assertions` stands in for "xrust is unoptimized", as in ikigai-store's
/// `sparql_stack_size`: it errs safe in both mixed profiles a host is likely to use.
pub const fn xslt_stack_size() -> usize {
    STACK_BASE
        + STACK_PER_ELEMENT * 2 * MAX_XML_DEPTH
        + STACK_PER_EXPR * MAX_EXPR_DEPTH
        + STACK_PER_RESULT_LEVEL * (XRUST_MAX_DEPTH + 1) * MAX_XML_DEPTH
}

/// How many idle [`on_xslt_stack`] threads are kept for the next call. Since 0.5.1 these run
/// no stylesheet compiles of this crate's own (those run on [`MAX_STYLESHEET_THREADS`]); they
/// are kept because starting a thread with [`xslt_stack_size`] of stack is not free.
pub const MAX_IDLE_THREADS: usize = 8;

/// How many stylesheets keep a thread of their own — **8**, the most recently used.
///
/// `transform_xml` and the endpoint run every transform through a stylesheet on that
/// stylesheet's one thread, which compiled it once, in arrival order (ledger #2826). Through
/// 0.5.0 each concurrent call took its own pooled thread and compiled the stylesheet again,
/// because a compiled stylesheet is `Rc` and cannot move between threads; and the compiles
/// and the runs all queued on one process-wide lock anyway, inside xrust's name interner
/// (`qualname` 0.0.2, a single `RwLock` taken for writing on every clone and drop of a name).
/// Measured with gonk's 140 KB stylesheet (`examples/concurrent-cost.rs`, release, 18 cores,
/// load average about 5): through 0.5.0 eight calls at once took 8.8 s for all and eight
/// compiles, against about 1.1 s for one cold call; eight warm calls at once took 1.06 s
/// against 0.11 s one after another, so the lock made concurrency nine times SLOWER than
/// taking turns. Since 0.5.1 the same eight cold calls take 1.14 s and one compile, and eight
/// warm ones 0.11 s. So running a
/// stylesheet's transforms one at a time costs no parallelism there was any of: while that
/// lock exists, transforms do not run in parallel, and this removes the recompiles and the
/// convoy, not the lock. Different stylesheets still run on different threads.
///
/// Past this many, the least recently used stylesheet's thread finishes what it was given and
/// exits, taking its compile with it; its next transform compiles again. A stylesheet's text
/// is its identity: an edited one is a different stylesheet and compiles once, and the old
/// one's thread ages out like any other.
pub const MAX_STYLESHEET_THREADS: usize = 8;

/// How many transforms may wait for one stylesheet's thread — **32**. One more is refused at
/// once with a transient `Unavailable`, rather than queued past its time budget. At gonk's warm
/// render (about 25 ms in release) a full queue is under a second of work.
pub const MAX_QUEUED_PER_STYLESHEET: usize = 32;

/// The time every transform this crate starts on its own gets: **5 seconds** — what
/// `urn:xslt:transform` gives every request, and what `transform_xml` and
/// `stylesheet_output_method` are answered within. Past it the caller gets a typed
/// [`Error::Timeout`] naming the budget, never a partial result (ledger #1040). The same
/// number as `ikigai-store`'s and `ikigai-sparql`'s SPARQL budgets, so one default holds
/// across the ecosystem's caller-supplied query languages.
///
/// The evidence, measured 2026-10-10 with xrust 2.2.0 in a release build on an 18-core
/// machine (`cargo run --release --example time-cost -- <gonk.xsl> <cms-web styles>`). COLD
/// is a stylesheet not yet compiled on that thread — what a caller sending its own stylesheet
/// always costs; WARM is the compile memoized:
///
/// | input | cold | warm |
/// | --- | --- | --- |
/// | gonk's 98 KB stylesheet, a 10-row queue chunk (what gonk renders: `CHUNK_ROWS`) | 0.73 s | 0.31 s |
/// | the same, 25 rows in one document | 1.2 s | 0.78 s |
/// | the same, 50 rows in one document | 2.0 s | 1.6 s |
/// | the same, 100 rows in one document † | 5.9 s | 5.8 s |
/// | the reading room's `catalog.xsl`, a 60-resource page (its page size) | 0.045 s | 0.031 s |
/// | the same, 600 resources | 0.90 s | 0.88 s |
/// | an XPath of 8,192 `+1` terms (16 KB of stylesheet) | 0.69 s | 0.004 s |
/// | the same, 16,384 terms (32 KB) | 1.9 s | 0.007 s |
/// | a template recursing to build an 800-level result (~600 bytes) | 0.49 s | 0.49 s |
/// | the same, 1,200 levels | 1.6 s | 1.6 s |
/// | the same, 1,592 levels | 3.7 s | 3.8 s |
///
/// † measured while another build pushed the load average from 7 to 35, so high.
///
/// So 5 s is about seven times gonk's cold chunk and a hundred times the reading room's page,
/// while the attacks are already past it a step or two up their curves: the operator chain
/// is in the COMPILE (warm, it is nothing), and the recursion grows about as the CUBE of the
/// result's depth, which xrust lets reach 199 calls of up to 60 nested elements each — minutes,
/// from a stylesheet under 1 KB. Nothing in either input's bytes or nesting bounds that, so
/// time is bounded as time.
///
/// ⚠ A host that renders larger single documents than these on purpose — gonk's 400-row
/// queue in one document took 23.6 s before it was chunked — calls `transform_xml_within`
/// (or [`on_xslt_stack_within`]) with its own budget. The endpoint has no host constructor
/// and no argument a budget could ride on, so its budget is this constant.
pub const DEFAULT_TIME_BUDGET: Duration = Duration::from_secs(5);

/// A quarter of this machine's available parallelism, and at least one: how many transforms
/// may still be running after their callers' budgets ran out before new ones are refused.
/// See [`on_xslt_stack_within`].
pub fn max_overdue_transforms() -> usize {
    static MAX: OnceLock<usize> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get() / 4)
            .unwrap_or(1)
            .max(1)
    })
}

/// How many transforms are running right now after their callers were answered with a
/// [`Error::Timeout`] — work xrust cannot be told to stop. Always 0 on wasm.
pub fn overdue_transforms() -> usize {
    #[cfg(not(target_family = "wasm"))]
    {
        pool::overdue()
    }
    #[cfg(target_family = "wasm")]
    {
        0
    }
}

/// Run `work` — a parse, compile and transform of inputs [`check_source`] and
/// [`check_stylesheet`] admitted — on a thread with [`xslt_stack_size`] of stack, and return
/// what it returns, however long it takes. [`on_xslt_stack_within`] is the same with a
/// deadline; everything this crate runs on its own uses that one.
///
/// ★ **A panic in `work` is answered, not resumed**: the caller gets [`Error::Endpoint`]
/// naming it (ledger #1040). xrust panics on some input a caller controls — `<xsl:copy-of
/// select="/"/>` (it cannot attach a document node to the result), an attribute at the top of
/// the result, an `xsl:sort` key that fails to evaluate — and through 0.2.0 that panic was
/// re-raised on the caller's thread, which in a host is a request handler. The default panic
/// hook still prints the panic's message to stderr: that hook is the host's, not this crate's.
///
/// The threads are pooled: a call takes an idle one (or starts one) and gives it back, keeping
/// at most [`MAX_IDLE_THREADS`]. A thread whose work panicked is not given back. This crate's
/// own transforms do not run here but on their stylesheet's thread ([`MAX_STYLESHEET_THREADS`]),
/// because a compiled stylesheet is memoized per THREAD (xrust's tree is `Rc`, so it cannot be
/// shared) and an arbitrary pooled thread would compile it again (ledger #453, #2826).
///
/// ⚠ On wasm there is no thread to start: `work` runs inline, on whatever stack the host gave
/// the module, so only the bounds protect it there, and a panic is whatever the module's
/// panic strategy makes it (an abort, on `wasm32-unknown-unknown`).
///
/// ```
/// let answer = ikigai_xslt::limits::on_xslt_stack(|| 6 * 7).unwrap();
/// assert_eq!(answer, 42);
/// let refused = ikigai_xslt::limits::on_xslt_stack(|| -> u8 { panic!("inside") });
/// assert!(matches!(refused, Err(ikigai_core::Error::Endpoint(m)) if m.contains("inside")));
/// ```
pub fn on_xslt_stack<T, F>(work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    #[cfg(not(target_family = "wasm"))]
    {
        pool::run(None, work)
    }
    #[cfg(target_family = "wasm")]
    {
        Ok(work())
    }
}

/// [`on_xslt_stack`], answered within `budget`: past it the caller gets a typed
/// [`Error::Timeout`] naming the budget — never a partial result (ledger #1040).
///
/// ⚠ **The work is ABANDONED, not cancelled.** xrust has no cancellation point — no hook is
/// called per step, and its only callbacks (`xsl:message`, `document()`, runtime parsing) are
/// ones a stylesheet need not reach — and there is no way to stop a Rust thread from outside
/// it. So the pooled thread runs the work to its end and then exits (it is never given back
/// to the pool). What the budget buys is that the CALLER is answered on time and an async
/// worker is not held. What bounds the CPU is a count: a transform still running after its
/// caller was answered is OVERDUE ([`overdue_transforms`]), and while
/// [`max_overdue_transforms`] are, every new call is refused at once with a transient
/// [`Error::Unavailable`] saying why — the store's answer to the same problem (ledger #964).
///
/// On wasm there is no thread, so there is no deadline: `work` runs inline, to its end.
///
/// ```
/// use std::time::Duration;
/// use ikigai_xslt::limits::on_xslt_stack_within;
///
/// assert_eq!(on_xslt_stack_within(Duration::from_secs(5), || 6 * 7).unwrap(), 42);
/// let late = on_xslt_stack_within(Duration::from_millis(10), || {
///     std::thread::sleep(Duration::from_millis(200));
/// });
/// assert!(matches!(late, Err(ikigai_core::Error::Timeout(m)) if m.contains("10 ms")));
/// ```
pub fn on_xslt_stack_within<T, F>(budget: Duration, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    #[cfg(not(target_family = "wasm"))]
    {
        pool::run(Some(budget), work)
    }
    #[cfg(target_family = "wasm")]
    {
        let _ = budget;
        Ok(work())
    }
}

/// Let every idle [`on_xslt_stack`] thread go (each exits). The next call starts a fresh one.
/// A no-op on wasm, which has none. The stylesheet threads are let go by
/// `clear_stylesheet_cache`.
pub fn release_idle_threads() {
    #[cfg(not(target_family = "wasm"))]
    pool::release_idle();
}

/// Run `work` on `stylesheet`'s own thread (see [`MAX_STYLESHEET_THREADS`]), after the
/// transforms of it already waiting, answered within `budget` from now — the wait counts.
/// Everything [`on_xslt_stack_within`] says about the stack, panics and overdue work holds,
/// with three differences:
///
/// * a call still waiting when its budget runs out is answered and skipped, never run, and is
///   not counted overdue;
/// * a call still RUNNING then is counted overdue as before, and the stylesheet gets a new
///   thread: the calls queued behind it move there (and wait for one compile) rather than
///   wait for work nobody is waiting for;
/// * after a panic the thread stays, and `reset` runs on it first (the crate passes the
///   function that forgets the thread's compiled stylesheet, so the next call recompiles
///   rather than trust it).
///
/// On wasm there is no thread: `work` runs inline, to its end.
pub(crate) fn on_stylesheet_thread_within<T, F>(
    stylesheet: &str,
    budget: Duration,
    reset: fn(),
    work: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    #[cfg(not(target_family = "wasm"))]
    {
        pool::run_keyed(&pool::HOMES, stylesheet, Some(budget), reset, work)
    }
    #[cfg(target_family = "wasm")]
    {
        let _ = (stylesheet, budget, reset);
        Ok(work())
    }
}

/// Let every stylesheet thread go: each finishes what it was given, then exits with its
/// compile. A no-op on wasm.
pub(crate) fn release_stylesheet_threads() {
    #[cfg(not(target_family = "wasm"))]
    pool::HOMES.release();
}

/// The refusal a transform past its budget is answered with.
#[cfg(not(target_family = "wasm"))]
fn timeout(budget: Duration) -> Error {
    Error::Timeout(format!(
        "this XSLT transform ran past its time budget of {} ms and its caller was answered \
         instead of waiting (ledger #1040); nothing it produced is returned. The XSLT engine \
         cannot be stopped partway, so it finishes on its own thread and is counted until it \
         does. Some shapes are super-linear in the engine — a long chain of operators, a \
         template that recurses to build a deep result, a path nested inside a predicate \
         over every node — so make the input or the stylesheet smaller, or, as the host, run \
         it with a larger budget",
        budget.as_millis()
    ))
}

/// What a caught panic carried, as text.
#[cfg(not(target_family = "wasm"))]
fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "a panic with no message".to_string())
}

#[cfg(not(target_family = "wasm"))]
mod pool {
    use super::{
        max_overdue_transforms, panic_message, timeout, xslt_stack_size, MAX_IDLE_THREADS,
        MAX_QUEUED_PER_STYLESHEET, MAX_STYLESHEET_THREADS,
    };
    use ikigai_core::{Error, Result};
    use std::collections::VecDeque;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    /// One unit of work for a pooled thread; it answers whether the thread may be reused.
    type Job = Box<dyn FnOnce() -> bool + Send>;

    /// What a job's work produced: its value, or the panic that ended it.
    type Outcome<T> = std::thread::Result<T>;

    /// The idle threads, each reached through the sender of its job queue.
    static IDLE: Mutex<Vec<Sender<Job>>> = Mutex::new(Vec::new());

    /// The stylesheet threads every transform of this crate's runs on.
    pub(super) static HOMES: Homes = Homes::new();

    /// Transforms still running after their callers were answered with a timeout.
    static OVERDUE: AtomicUsize = AtomicUsize::new(0);

    pub(super) fn overdue() -> usize {
        OVERDUE.load(Ordering::SeqCst)
    }

    /// Where one call stands, shared by its worker and the caller waiting for it.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        /// Handed to a thread that has not started it: a caller that gives up now costs
        /// nothing, because the thread skips it.
        Queued,
        Running,
        /// The worker finished while the caller was still waiting: the caller takes its
        /// answer, whatever the clock says by the time it looks.
        Settled,
        /// The caller gave up. If the work was running then, it was counted overdue, and its
        /// finish uncounts it; if it was still queued, it is skipped.
        Abandoned,
    }

    fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        // Every guarded value (vectors of senders and homes, a queue of boxed jobs, a plain
        // enum) is valid whatever a panic interrupted, so poisoning carries no meaning here.
        m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A transient refusal when too many abandoned transforms are still running.
    fn refuse_if_overdue() -> Result<()> {
        let (running, most) = (overdue(), max_overdue_transforms());
        if running >= most {
            return Err(Error::Unavailable(format!(
                "{running} XSLT transforms are still running after their callers' time \
                 budgets ran out (the most allowed is {most}, a quarter of this machine's \
                 cores); the engine cannot be stopped partway, so new transforms are \
                 refused until one finishes, rather than letting each request hold \
                 another core (ledger #1040). Retry shortly"
            )));
        }
        Ok(())
    }

    /// Wrap `work` as a job, with the state its caller and its worker share and the channel
    /// its outcome comes back on. `on_panic` runs on the worker after `work` panicked.
    fn wrap<T, F>(work: F, on_panic: Option<fn()>) -> (Job, Arc<Mutex<State>>, Receiver<Outcome<T>>)
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let state = Arc::new(Mutex::new(State::Queued));
        let (answer, reply) = sync_channel(1);
        let worker_state = Arc::clone(&state);
        let job: Job = Box::new(move || {
            {
                let mut state = lock(&worker_state);
                if *state == State::Abandoned {
                    // Its caller gave up before it started: nothing to run, nothing counted.
                    return false;
                }
                *state = State::Running;
            }
            let outcome = catch_unwind(AssertUnwindSafe(work));
            let abandoned = {
                let mut state = lock(&worker_state);
                if *state == State::Abandoned {
                    true
                } else {
                    *state = State::Settled;
                    false
                }
            };
            if abandoned {
                // Uncounted only now that it has actually finished.
                OVERDUE.fetch_sub(1, Ordering::SeqCst);
                return false;
            }
            let reusable = outcome.is_ok();
            if !reusable {
                if let Some(reset) = on_panic {
                    reset();
                }
            }
            let _ = answer.send(outcome);
            reusable
        });
        (job, state, reply)
    }

    /// Wait for a job's outcome, within `budget` if there is one. A job abandoned while it
    /// runs is counted overdue and `overdue_hook` runs (after the state's lock is let go).
    fn wait<T>(
        budget: Option<Duration>,
        state: &Mutex<State>,
        reply: &Receiver<Outcome<T>>,
        overdue_hook: impl FnOnce(),
    ) -> Result<Outcome<T>> {
        let lost = || Error::Unavailable("the XSLT thread exited without an answer".to_string());
        let Some(budget) = budget else {
            return reply.recv().map_err(|_| lost());
        };
        match reply.recv_timeout(budget) {
            Ok(outcome) => Ok(outcome),
            Err(RecvTimeoutError::Timeout) => {
                let was = {
                    let mut state = lock(state);
                    let was = *state;
                    if matches!(was, State::Queued | State::Running) {
                        *state = State::Abandoned;
                    }
                    was
                };
                match was {
                    State::Queued => Err(timeout(budget)),
                    State::Running => {
                        OVERDUE.fetch_add(1, Ordering::SeqCst);
                        overdue_hook();
                        Err(timeout(budget))
                    }
                    // Settled: it finished in time and its answer is on the way.
                    State::Settled | State::Abandoned => reply.recv().map_err(|_| lost()),
                }
            }
            Err(RecvTimeoutError::Disconnected) => Err(lost()),
        }
    }

    /// A job's outcome as the caller's answer: a panic becomes an `Endpoint` error.
    fn answer<T>(outcome: Outcome<T>) -> Result<T> {
        outcome.map_err(|panic| {
            Error::Endpoint(format!(
                "the XSLT engine (xrust) panicked on this input instead of answering: {}. \
                 It is an input the engine cannot run — for example copying the document \
                 node or an attribute to the top of the result, or an xsl:sort key that \
                 fails to evaluate — and nothing was produced",
                panic_message(&*panic)
            ))
        })
    }

    fn start() -> Result<Sender<Job>> {
        let (jobs, queue) = channel::<Job>();
        std::thread::Builder::new()
            .name("ikigai-xslt".to_string())
            .stack_size(xslt_stack_size())
            .spawn(move || {
                for job in queue {
                    if !job() {
                        break;
                    }
                }
            })
            .map_err(|e| {
                Error::Unavailable(format!(
                    "could not start a {} MiB thread to run this XSLT transform on: {e}",
                    xslt_stack_size() >> 20
                ))
            })?;
        Ok(jobs)
    }

    pub(super) fn release_idle() {
        // Dropping a sender ends that thread's job loop.
        lock(&IDLE).clear();
    }

    pub(super) fn run<T, F>(budget: Option<Duration>, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if budget.is_some() {
            refuse_if_overdue()?;
        }
        let (mut job, state, reply) = wrap(work, None);
        // An idle thread can have exited (its queue then refuses the job and hands it back);
        // fall through to a fresh one rather than fail the call.
        let worker = loop {
            let Some(worker) = lock(&IDLE).pop() else {
                let worker = start()?;
                if worker.send(job).is_err() {
                    return Err(Error::Unavailable(
                        "a new XSLT thread exited before it took the work".to_string(),
                    ));
                }
                break worker;
            };
            match worker.send(job) {
                Ok(()) => break worker,
                Err(refused) => job = refused.0,
            }
        };
        // On a timeout `worker` drops when this returns, so the thread exits after this job.
        let outcome = wait(budget, &state, &reply, || {})?;
        if outcome.is_ok() {
            let mut idle = lock(&IDLE);
            if idle.len() < MAX_IDLE_THREADS {
                idle.push(worker);
            }
        }
        answer(outcome)
    }

    /// One stylesheet's thread: it compiles the stylesheet once (in its memo) and runs every
    /// transform through it, one after another, in the order they arrived (ledger #2826).
    struct Home {
        /// The stylesheet's full text: the compile is a pure function of it.
        key: Arc<str>,
        queue: Mutex<Queue>,
        ready: Condvar,
    }

    struct Queue {
        jobs: VecDeque<Job>,
        /// No new work comes here: the thread finishes what is queued and exits.
        retired: bool,
    }

    fn start_home(key: &str) -> Result<Arc<Home>> {
        let home = Arc::new(Home {
            key: Arc::from(key),
            queue: Mutex::new(Queue {
                jobs: VecDeque::new(),
                retired: false,
            }),
            ready: Condvar::new(),
        });
        let thread_home = Arc::clone(&home);
        std::thread::Builder::new()
            .name("ikigai-xslt-stylesheet".to_string())
            .stack_size(xslt_stack_size())
            .spawn(move || loop {
                let job = {
                    let mut queue = lock(&thread_home.queue);
                    loop {
                        if let Some(job) = queue.jobs.pop_front() {
                            break job;
                        }
                        if queue.retired {
                            // Its memo, and the compiled stylesheet in it, go with the thread.
                            return;
                        }
                        queue = thread_home
                            .ready
                            .wait(queue)
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                    }
                };
                // A panic was caught inside the job, which reset the memo; the thread stays.
                job();
            })
            .map_err(|e| {
                Error::Unavailable(format!(
                    "could not start a {} MiB thread to run this XSLT stylesheet on: {e}",
                    xslt_stack_size() >> 20
                ))
            })?;
        Ok(home)
    }

    /// Mark `home` retired and wake its thread; with `take`, hand back what it had queued.
    fn retire(home: &Home, take: bool) -> VecDeque<Job> {
        let pending = {
            let mut queue = lock(&home.queue);
            queue.retired = true;
            if take {
                std::mem::take(&mut queue.jobs)
            } else {
                VecDeque::new()
            }
        };
        home.ready.notify_all();
        pending
    }

    /// Stylesheet threads, most recently used first, at most [`MAX_STYLESHEET_THREADS`]. A
    /// value rather than only the one static so a test can have a set no other test touches.
    /// Lock order: this, then a home's queue; a home's thread takes only its queue.
    pub(super) struct Homes(Mutex<Vec<Arc<Home>>>);

    impl Homes {
        pub(super) const fn new() -> Self {
            Homes(Mutex::new(Vec::new()))
        }

        /// Queue `job` on `key`'s home, starting one (and retiring the least recently used
        /// past [`MAX_STYLESHEET_THREADS`]) if there is none.
        fn enqueue(&self, key: &str, job: Job) -> Result<Arc<Home>> {
            Self::enqueue_in(&mut lock(&self.0), key, job)
        }

        fn enqueue_in(homes: &mut Vec<Arc<Home>>, key: &str, job: Job) -> Result<Arc<Home>> {
            let home = match homes.iter().position(|h| &*h.key == key) {
                Some(i) => {
                    let home = homes.remove(i);
                    homes.insert(0, Arc::clone(&home));
                    home
                }
                None => {
                    let home = start_home(key)?;
                    homes.insert(0, Arc::clone(&home));
                    if homes.len() > MAX_STYLESHEET_THREADS {
                        for evicted in homes.split_off(MAX_STYLESHEET_THREADS) {
                            // It finishes what it was given, then exits.
                            retire(&evicted, false);
                        }
                    }
                    home
                }
            };
            {
                let mut queue = lock(&home.queue);
                if queue.jobs.len() >= MAX_QUEUED_PER_STYLESHEET {
                    return Err(Error::Unavailable(format!(
                        "{} XSLT transforms of this stylesheet are already waiting their turn \
                         (the most allowed is {MAX_QUEUED_PER_STYLESHEET}); transforms of one \
                         stylesheet run one at a time on the thread that compiled it (ledger \
                         #2826), so this one is refused rather than queued past its time \
                         budget. Retry shortly",
                        queue.jobs.len()
                    )));
                }
                queue.jobs.push_back(job);
            }
            home.ready.notify_one();
            Ok(home)
        }

        /// A transform outlived its caller on `home`: give the stylesheet a new home, and move
        /// every call queued behind the overdue one there, so they wait for a compile rather
        /// than for work nobody is waiting for. The old thread finishes that work and exits.
        /// Done whether or not `home` is still listed: one already let go (evicted or
        /// released) is still draining its queue, and the calls in it should not wait either.
        fn rehome(&self, home: &Arc<Home>) {
            let mut homes = lock(&self.0);
            if let Some(i) = homes.iter().position(|h| Arc::ptr_eq(h, home)) {
                homes.remove(i);
            }
            for job in retire(home, true) {
                // A job that cannot be placed is dropped, and its caller is answered that the
                // thread exited without an answer.
                let _ = Self::enqueue_in(&mut homes, &home.key, job);
            }
        }

        /// Let every thread go: each finishes what it was given, then exits.
        pub(super) fn release(&self) {
            for home in lock(&self.0).drain(..) {
                retire(&home, false);
            }
        }

        /// How many calls wait for `key`'s thread (not counting the one it is running).
        #[cfg(test)]
        pub(super) fn queued(&self, key: &str) -> usize {
            lock(&self.0)
                .iter()
                .find(|h| &*h.key == key)
                .map_or(0, |h| lock(&h.queue).jobs.len())
        }
    }

    pub(super) fn run_keyed<T, F>(
        homes: &'static Homes,
        key: &str,
        budget: Option<Duration>,
        reset: fn(),
        work: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if budget.is_some() {
            refuse_if_overdue()?;
        }
        let (job, state, reply) = wrap(work, Some(reset));
        let home = homes.enqueue(key, job)?;
        let outcome = wait(budget, &state, &reply, || homes.rehome(&home))?;
        answer(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(text: &str) -> Shape {
        scan(text).unwrap()
    }

    #[test]
    fn elements_nest_and_self_closed_ones_count_a_level() {
        assert_eq!(shape("<a><b/><c><d>x</d></c></a>").depth, 3);
        assert_eq!(shape("<a/>").depth, 1);
        assert_eq!(shape("text, no tags").depth, 0);
        assert_eq!(shape(&"<a>".repeat(400)).depth, 400);
        // A closing tag closes nothing below zero: it cannot be used to look shallower.
        assert_eq!(shape("</a></a><a><a></a></a>").depth, 2);
    }

    #[test]
    fn quotes_comments_cdata_and_instructions_hide_tags() {
        assert_eq!(shape(r#"<a t="1>2"><b u='<c><c>'></b></a>"#).depth, 2);
        assert_eq!(shape("<a><!-- <b><b><b> --></a>").depth, 1);
        assert_eq!(shape("<a><![CDATA[<b><b><b>]]></a>").depth, 1);
        assert_eq!(shape("<?xml version='1.0'?><a><?pi <b><b> ?></a>").depth, 1);
        // An unterminated comment hides the rest, and xrust refuses the document.
        assert_eq!(shape("<a><!-- <b><b>").depth, 1);
    }

    #[test]
    fn brackets_in_attribute_values_are_expression_depth() {
        let s = shape(r#"<t select="count(//a[b[c]])" test='{((1))}'/>"#);
        assert_eq!(s.expr_depth, 3);
        // Brackets in text are not an expression.
        assert_eq!(shape("<t>((((((((</t>").expr_depth, 0);
        // Each attribute value is its own expression: depth does not carry across.
        assert_eq!(shape(r#"<t a="(((" b="((("/>"#).expr_depth, 3);
        // Character references that decode to brackets count as brackets.
        assert_eq!(shape(r#"<t a="&#40;&#x28;&#91;&#x7B;1"/>"#).expr_depth, 4);
    }

    #[test]
    fn a_namespace_entity_is_admitted_and_counted_as_its_expansion() {
        let doc = r#"<?xml version="1.0"?>
<!DOCTYPE rdf:RDF [
  <!-- the usual RDF/XML shorthand -->
  <!ENTITY rdf "http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <!ENTITY ex 'http://example.org/(x)'>
  <!ELEMENT rdf:RDF ANY>
  <!ATTLIST rdf:RDF a CDATA "x>y">
]>
<rdf:RDF><a rdf:about="&rdf;type" b="&ex;&ex;">&rdf;</a></rdf:RDF>"#;
        let s = shape(doc);
        assert_eq!(s.depth, 2);
        assert_eq!(s.entity_expansion, 43 * 2 + 22 * 2);
        // `&ex;` holds one `(`: it counts as an opening bracket each time, closing nothing.
        assert_eq!(s.expr_depth, 2);
    }

    #[test]
    fn entities_that_could_build_markup_or_multiply_are_refused() {
        for (decl, why) in [
            (r#"<!ENTITY e "<a><a><a>">"#, "`<`"),
            (r#"<!ENTITY e "&f;&f;">"#, "`&`"),
            (r#"<!ENTITY e "&#60;a&#62;">"#, "`&`"),
            (r#"<!ENTITY e "%p;">"#, "`%`"),
            (r#"<!ENTITY % p "x">"#, "parameter entity"),
            (r#"<!ENTITY e SYSTEM "file:///etc/passwd">"#, "external"),
            (r#"<!ENTITY e PUBLIC "-//x" "http://x/">"#, "external"),
            ("%p;", "parameter entity"),
        ] {
            let doc = format!("<!DOCTYPE d [ {decl} ]><d/>");
            let err = scan(&doc).unwrap_err();
            assert!(err.contains(why), "{decl}: {err}");
        }
        // A DOCTYPE with only an external identifier declares nothing here.
        assert_eq!(
            shape(r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN" "x.dtd"><html/>"#)
                .depth,
            1
        );
    }

    #[test]
    fn a_flat_entity_bomb_is_refused_by_its_expansion() {
        let value = "x".repeat(10_000);
        let doc = format!(
            "<!DOCTYPE d [<!ENTITY e \"{value}\">]><d>{}</d>",
            "&e;".repeat(200)
        );
        let err = scan(&doc).unwrap_err();
        assert!(err.contains("MAX_ENTITY_EXPANSION"), "{err}");
    }

    #[test]
    fn the_checks_refuse_by_name_and_admit_at_the_bound() {
        let at = format!(
            "{}{}",
            "<a>".repeat(MAX_XML_DEPTH),
            "</a>".repeat(MAX_XML_DEPTH)
        );
        check_source(&at, "src").unwrap();
        let over = format!("<b>{at}</b>");
        let err = check_source(&over, "content").unwrap_err();
        assert!(
            matches!(&err, Error::InvalidArgument { name, detail }
                if name == "content" && detail.contains("MAX_XML_DEPTH")),
            "{err}"
        );
        let expr = |n: usize| format!("<t s=\"{}1{}\"/>", "(".repeat(n), ")".repeat(n));
        check_stylesheet(&expr(MAX_EXPR_DEPTH), "stylesheet").unwrap();
        // A source document's attributes are data: no expression bound.
        check_source(&expr(MAX_EXPR_DEPTH + 1), "src").unwrap();
        let err = check_stylesheet(&expr(MAX_EXPR_DEPTH + 1), "stylesheet").unwrap_err();
        assert!(
            matches!(&err, Error::InvalidArgument { name, detail }
                if name == "stylesheet" && detail.contains("MAX_EXPR_DEPTH")),
            "{err}"
        );
        let err = check_source("<!DOCTYPE d [<!ENTITY e \"<a>\">]><d/>", "src").unwrap_err();
        assert!(matches!(&err, Error::InvalidArgument { name, .. } if name == "src"));
    }

    #[test]
    fn a_panic_is_answered_on_the_caller_and_the_pool_keeps_working() {
        let caught = std::panic::catch_unwind(|| on_xslt_stack(|| -> u8 { panic!("inside") }));
        let answer = caught.expect("a panic in the work is answered, never resumed");
        assert!(
            matches!(&answer, Err(Error::Endpoint(m)) if m.contains("inside")),
            "{answer:?}"
        );
        let within = on_xslt_stack_within(Duration::from_secs(60), || -> u8 { panic!("again") });
        assert!(matches!(&within, Err(Error::Endpoint(m)) if m.contains("again")));
        assert_eq!(on_xslt_stack(|| 1 + 1).unwrap(), 2);
        // Calls from many threads at once each get an answer.
        let handles: Vec<_> = (0..16)
            .map(|i| std::thread::spawn(move || on_xslt_stack(move || i * 2).unwrap()))
            .collect();
        let sum: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(sum, (0..16).map(|i| i * 2).sum::<usize>());
        release_idle_threads();
        assert_eq!(on_xslt_stack(|| 3).unwrap(), 3);
    }

    /// The stylesheet threads (ledger #2826), on a set of their own so no other test's
    /// stylesheets can evict these mid-test.
    #[test]
    fn a_stylesheet_thread_runs_its_calls_in_turn_and_drops_what_nobody_waits_for() {
        use super::pool::{run_keyed, Homes};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
        use std::sync::{mpsc, Arc};
        use std::thread::{self, JoinHandle, ThreadId};
        use std::time::Instant;

        static HOMES: Homes = Homes::new();
        static RESETS: AtomicUsize = AtomicUsize::new(0);
        fn reset() {
            RESETS.fetch_add(1, SeqCst);
        }
        fn me() -> ThreadId {
            thread::current().id()
        }
        fn call<T: Send + 'static>(
            key: &'static str,
            budget: Duration,
            work: impl FnOnce() -> T + Send + 'static,
        ) -> JoinHandle<Result<T>> {
            thread::spawn(move || run_keyed(&HOMES, key, Some(budget), reset, work))
        }
        fn until(what: &str, done: impl Fn() -> bool) {
            let start = Instant::now();
            while !done() {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "waited for {what}"
                );
                thread::sleep(Duration::from_millis(5));
            }
        }
        /// A flag for a call's work to raise when it starts: one handle to read, one to raise.
        fn flag() -> (Arc<AtomicBool>, Arc<AtomicBool>) {
            let f = Arc::new(AtomicBool::new(false));
            (Arc::clone(&f), f)
        }
        let long = Duration::from_secs(30);

        // 1. Every call of one stylesheet runs on its one thread, never two at once.
        let inside = Arc::new(AtomicUsize::new(0));
        let calls: Vec<_> = (0..8)
            .map(|_| {
                let inside = Arc::clone(&inside);
                call("one", long, move || {
                    assert_eq!(inside.fetch_add(1, SeqCst), 0, "two calls ran at once");
                    thread::sleep(Duration::from_millis(10));
                    inside.fetch_sub(1, SeqCst);
                    me()
                })
            })
            .collect();
        let threads: Vec<ThreadId> = calls
            .into_iter()
            .map(|c| c.join().unwrap().unwrap())
            .collect();
        assert!(threads.iter().all(|t| *t == threads[0]), "{threads:?}");

        // 2. A call whose budget runs out while it WAITS is answered then, never run, and never
        //    counted overdue.
        let (started, raised) = flag();
        let slow = call("two", long, move || {
            raised.store(true, SeqCst);
            thread::sleep(Duration::from_millis(600));
        });
        until("the slow call to start", || started.load(SeqCst));
        let (ran, ran_flag) = flag();
        let start = Instant::now();
        let late = run_keyed(
            &HOMES,
            "two",
            Some(Duration::from_millis(100)),
            reset,
            move || ran_flag.store(true, SeqCst),
        );
        assert!(matches!(late, Err(Error::Timeout(_))), "{late:?}");
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(
            overdue_transforms(),
            0,
            "a call that never started is not overdue"
        );
        slow.join().unwrap().unwrap();
        assert_eq!(
            run_keyed(&HOMES, "two", Some(long), reset, || 2).unwrap(),
            2
        );
        assert!(!ran.load(SeqCst), "a call nobody waited for still ran");

        // 3. A call still RUNNING at its budget is overdue, and the calls queued behind it move
        //    to a new thread rather than wait for it.
        let (started, raised) = flag();
        let (tx, rx) = mpsc::channel();
        let overdue = call("three", Duration::from_millis(300), move || {
            tx.send(me()).unwrap();
            raised.store(true, SeqCst);
            thread::sleep(Duration::from_millis(2000));
        });
        until("the overdue call to start", || started.load(SeqCst));
        let start = Instant::now();
        let behind = call("three", long, me);
        until("a call to queue behind it", || HOMES.queued("three") == 1);
        let moved = behind.join().unwrap().unwrap();
        assert!(
            start.elapsed() < Duration::from_millis(1500),
            "the queued call waited {:?} for the overdue one",
            start.elapsed()
        );
        assert_ne!(moved, rx.recv().unwrap(), "it ran on a new thread");
        assert!(matches!(overdue.join().unwrap(), Err(Error::Timeout(_))));
        assert_eq!(overdue_transforms(), 1);
        until("the overdue work to finish", || overdue_transforms() == 0);

        // 4. A panic is answered; the thread stays, forgets its compile (`reset`), and runs the
        //    call queued behind it.
        let (started, raised) = flag();
        let (tx, rx) = mpsc::channel();
        let panics = call("four", long, move || -> ThreadId {
            tx.send(me()).unwrap();
            raised.store(true, SeqCst);
            thread::sleep(Duration::from_millis(200));
            panic!("inside")
        });
        until("the panicking call to start", || started.load(SeqCst));
        let next = call("four", long, me);
        until("a call to queue behind it", || HOMES.queued("four") == 1);
        let caught = panics.join().unwrap();
        assert!(matches!(&caught, Err(Error::Endpoint(m)) if m.contains("inside")));
        assert_eq!(RESETS.load(SeqCst), 1);
        assert_eq!(next.join().unwrap().unwrap(), rx.recv().unwrap());

        // 5. Past MAX_QUEUED_PER_STYLESHEET waiting, a call is refused at once.
        let (started, raised) = flag();
        let busy = call("five", long, move || {
            raised.store(true, SeqCst);
            thread::sleep(Duration::from_millis(800));
        });
        until("the busy call to start", || started.load(SeqCst));
        let waiting: Vec<_> = (0..MAX_QUEUED_PER_STYLESHEET)
            .map(|i| call("five", long, move || i))
            .collect();
        until("the queue to fill", || {
            HOMES.queued("five") == MAX_QUEUED_PER_STYLESHEET
        });
        let refused = run_keyed(&HOMES, "five", Some(long), reset, || 0);
        assert!(
            matches!(&refused, Err(Error::Unavailable(m)) if m.contains("already waiting")),
            "{refused:?}"
        );
        busy.join().unwrap().unwrap();
        let answered: usize = waiting
            .into_iter()
            .map(|w| w.join().unwrap().unwrap())
            .sum();
        assert_eq!(answered, (0..MAX_QUEUED_PER_STYLESHEET).sum::<usize>());

        // 6. Past MAX_STYLESHEET_THREADS stylesheets the least recently used one lets go, and
        //    its next call starts a new thread.
        let keys: Vec<&'static str> = (0..=MAX_STYLESHEET_THREADS)
            .map(|i| &*Box::leak(format!("six-{i}").into_boxed_str()))
            .collect();
        let first = run_keyed(&HOMES, keys[0], Some(long), reset, me).unwrap();
        assert_eq!(
            run_keyed(&HOMES, keys[0], Some(long), reset, me).unwrap(),
            first
        );
        for key in &keys[1..] {
            run_keyed(&HOMES, key, Some(long), reset, me).unwrap();
        }
        assert_ne!(
            run_keyed(&HOMES, keys[0], Some(long), reset, me).unwrap(),
            first
        );
        HOMES.release();
    }
}
