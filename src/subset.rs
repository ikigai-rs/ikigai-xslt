//! The constructs xrust answers WRONGLY, with no error, refused before it compiles them
//! (ledger #193).
//!
//! xrust 2.2.0 evaluates part of XSLT 1.0 to a wrong answer and says nothing: a predicate in a
//! match pattern matches everything, `item[1]` selects every `item`, `position()` and `last()`
//! are 1, a leading `//` starts from the context node, and a prefixed name inside an attribute
//! value template is empty. `tests/xrust_subset.rs` measures each one, through this crate and
//! through stock xrust. A stylesheet's author cannot discover them from errors, and a bound
//! must refuse rather than truncate, so compiling reads the parsed stylesheet before xrust
//! compiles it and refuses every one it can see, as a typed `InvalidArgument` naming the
//! argument, the construct, the element and attribute it is in, and the README row (the
//! "XSLT subset" section) that says what to write instead.
//!
//! ## What the scan sees, and what it does not
//!
//! The scan is **lexical and conservative**: it refuses only the shapes measured wrong, so it
//! never refuses a stylesheet xrust would have answered correctly by accident of a lexer.
//! Every XPath it reads is tokenized first, so a `[`, a `//` or a `prefix:name` inside a string
//! literal (`'a[1]'`, `"//"`) is text, not a construct.
//!
//! | construct | refused where | [`Construct`] |
//! | --- | --- | --- |
//! | a predicate (`[`) in a match pattern | `match` of `xsl:template` and `xsl:key` | [`Construct::MatchPredicate`] |
//! | a numeric predicate, `[1]` or `[last()]` | any expression, attribute value templates included | [`Construct::NumericPredicate`] |
//! | `position()` or `last()` | outside a predicate, where the context node is not the root; and `last()` inside any predicate | [`Construct::PositionOrLast`] |
//! | a leading `//` | where the context node is not the root, a predicate included | [`Construct::LeadingDoubleSlash`] |
//! | a prefixed name inside `{…}` | an attribute value template on a literal result element | [`Construct::PrefixedNameInAvt`] |
//!
//! "Where the context node is the root" is the one place two of those constructs are right:
//! the body of `xsl:template match="/"` outside any `xsl:for-each`, and a top-level
//! `xsl:variable` or `xsl:param`. Everywhere else — a template matching anything else, a named
//! template (its caller decides its context), the body of an `xsl:for-each`, an `xsl:sort` key,
//! an `xsl:key`'s `use` — the context is some other node, and xrust's answer is wrong there.
//!
//! Not refused, on purpose: `xsl:value-of` over several nodes, which xrust concatenates where
//! XSLT 1.0 takes the first. Whether a path selects one node or several is a property of the
//! DATA (`ik:title` is usually one node and may be three), so no lexical rule tells the two
//! apart: refusing a multi-step path would refuse `../@title`, which is always one node, and
//! still admit `item`, which is often several. It stays a README row with its workaround.
//!
//! ```
//! let style = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
//!   <xsl:template match="item[@k='2']"><hit/></xsl:template>
//! </xsl:stylesheet>"#;
//! let found = ikigai_xslt::subset::silent_constructs(style).unwrap();
//! assert_eq!(found.len(), 1);
//! assert_eq!(found[0].construct, ikigai_xslt::subset::Construct::MatchPredicate);
//! assert_eq!((found[0].element.as_str(), found[0].attribute.as_str()), ("xsl:template", "match"));
//!
//! // Refused when it is compiled, by name:
//! let err = ikigai_xslt::transform_xml("<doc/>", style, false).unwrap_err();
//! assert!(err.starts_with("invalid argument `stylesheet`:"), "{err}");
//! assert!(err.contains("a predicate in a match pattern"), "{err}");
//! ```

use ikigai_core::Error;
use xrust::item::Node;

use crate::node_id::IdNode;
use crate::XSL_NS;

/// The xrust release every construct here was measured on, and the README and
/// `docs/upstream-xrust.md` name. A RECORD, not a gate: a newer xrust on crates.io is a
/// warning in the ecosystem's daily scan, which says this table is due a re-measure (ledger
/// #193). `tests/xrust_subset.rs` re-measures it on whatever xrust the build resolved, and a
/// changed answer fails there.
pub const XRUST_MEASURED_ON: &str = "2.2.0";

/// One construct xrust answers wrongly without an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    /// A predicate in a match pattern: `match="item[@k='2']"` matches every `item`, and so
    /// template priority never prefers it either.
    MatchPredicate,
    /// A numeric predicate, `[1]` or `[last()]`: ignored, so every node is selected.
    NumericPredicate,
    /// `position()` or `last()` where they do not mean what XSLT says: always 1 outside a
    /// predicate when the context is not the root, and `last()` is 1 inside a predicate too.
    PositionOrLast,
    /// A leading `//` where the context is not the root: evaluated from the context node.
    LeadingDoubleSlash,
    /// A prefixed name inside an attribute value template on a literal result element:
    /// `href="{@rdf:about}"` is empty.
    PrefixedNameInAvt,
}

impl Construct {
    /// The README row this construct is, word for word (the first column of the "Refused
    /// before it compiles" table in "The XSLT subset").
    pub fn readme_row(self) -> &'static str {
        match self {
            Construct::MatchPredicate => "a predicate in a match pattern",
            Construct::NumericPredicate => "a numeric predicate: [1], [last()]",
            Construct::PositionOrLast => "position() or last() away from the root",
            Construct::LeadingDoubleSlash => "a leading // away from the root",
            Construct::PrefixedNameInAvt => "a prefixed name in an attribute value template",
        }
    }

    /// What to write instead, as the README says.
    pub fn instead(self) -> &'static str {
        match self {
            Construct::MatchPredicate => {
                "match the name alone and branch inside with xsl:if or xsl:choose"
            }
            Construct::NumericPredicate => "[position() = 1]",
            Construct::PositionOrLast => {
                "compute it outside and hand it in as data, or use position() inside a predicate"
            }
            Construct::LeadingDoubleSlash => "an absolute path from the root, such as /doc//name",
            Construct::PrefixedNameInAvt => {
                "<xsl:attribute name=\"…\"><xsl:value-of select=\"…\"/></xsl:attribute>"
            }
        }
    }
}

/// Where one [`Construct`] was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Silent {
    pub construct: Construct,
    /// The element it is on, as written in the stylesheet (`xsl:template`, `a`).
    pub element: String,
    /// The attribute it is in (`match`, `select`, `href`).
    pub attribute: String,
    /// That attribute's whole value.
    pub value: String,
}

impl Silent {
    /// The refusal's detail: the construct, where it is, the README row, and what to write.
    pub fn detail(&self) -> String {
        format!(
            "refused: {}=\"{}\" on <{}> holds \"{}\", which xrust {XRUST_MEASURED_ON} answers \
             wrongly and without an error (README, \"The XSLT subset\"); instead, write {}",
            self.attribute,
            self.value,
            self.element,
            self.construct.readme_row(),
            self.construct.instead()
        )
    }
}

/// Every construct in `stylesheet_xml` xrust would answer wrongly without an error, in
/// document order — empty when there is none. For a host or an author that wants the whole
/// list at once; compiling refuses on the first. Checks [`crate::limits::check_stylesheet`]'s
/// bounds and parses on [`crate::limits::on_xslt_stack_within`], like
/// [`crate::stylesheet_output_method`].
pub fn silent_constructs(stylesheet_xml: &str) -> Result<Vec<Silent>, String> {
    crate::limits::check_stylesheet(stylesheet_xml, "stylesheet").map_err(|e| e.to_string())?;
    let stylesheet_xml = stylesheet_xml.to_string();
    crate::limits::on_xslt_stack_within(crate::limits::DEFAULT_TIME_BUDGET, move || {
        let doc = crate::parse_xml(&stylesheet_xml)
            .map_err(|e| format!("stylesheet parse error: {}", e.message))?;
        Ok(scan(&doc))
    })
    .map_err(|e| e.to_string())?
}

/// Refuse the first construct the parsed stylesheet holds, as an `InvalidArgument` naming
/// `stylesheet`. Called by [`crate::CompiledStylesheet::compile`] before xrust compiles it.
pub(crate) fn check(doc: &IdNode) -> Result<(), Error> {
    match scan(doc).into_iter().next() {
        None => Ok(()),
        Some(silent) => Err(Error::InvalidArgument {
            name: "stylesheet".to_string(),
            detail: silent.detail(),
        }),
    }
}

/// Every construct, in document order.
fn scan(doc: &IdNode) -> Vec<Silent> {
    let mut found = Vec::new();
    // Explicit stack, no recursion: (element, whether its context node is the root).
    // The stylesheet's top level is evaluated with the root as context.
    let mut stack: Vec<(IdNode, bool)> = doc
        .child_iter()
        .filter(|c| c.is_element())
        .map(|c| (c, true))
        .collect();
    stack.reverse();
    while let Some((element, at_root)) = stack.pop() {
        let children_at_root = visit(&element, at_root, &mut found);
        let mut children: Vec<(IdNode, bool)> = element
            .child_iter()
            .filter(|c| c.is_element())
            .map(|c| (c, children_at_root))
            .collect();
        children.reverse();
        stack.extend(children);
    }
    found
}

/// Check one element's attributes, and say whether its children's context is the root.
fn visit(element: &IdNode, at_root: bool, found: &mut Vec<Silent>) -> bool {
    let Some(name) = element.name() else {
        return at_root;
    };
    let shown = element.to_prefixed_name();
    let in_xsl = name
        .namespace_uri()
        .is_some_and(|ns| ns.to_string() == XSL_NS);
    // (the attribute's name as written, whether it is in no namespace, its value)
    let attrs: Vec<(String, bool, String)> = element
        .attribute_iter()
        .filter_map(|a| {
            let n = a.name()?;
            let local = n.local_name().to_string();
            let (shown, plain) = match n.namespace_uri() {
                None => (local, true),
                // An attribute in the XSL namespace on a literal result element
                // (`xsl:use-attribute-sets`) is not an attribute value template.
                Some(ns) if ns.to_string() == XSL_NS => return None,
                // Named through the ELEMENT's in-scope namespaces: xrust's own
                // `to_prefixed_name` on an attribute node prints a debugging line to stderr.
                Some(ns) => {
                    let prefix = element.namespace_iter().find_map(|d| {
                        (d.as_namespace_uri().ok()? == &ns)
                            .then(|| {
                                d.as_namespace_prefix()
                                    .ok()
                                    .flatten()
                                    .map(|p| p.to_string())
                            })
                            .flatten()
                    });
                    match prefix {
                        Some(p) => (format!("{p}:{local}"), false),
                        None => (format!("{{{}}}{local}", ns.to_string()), false),
                    }
                }
            };
            Some((shown, plain, a.value().to_string()))
        })
        .collect();
    let mut report = |construct: Construct, attribute: &str, value: &str| {
        found.push(Silent {
            construct,
            element: shown.clone(),
            attribute: attribute.to_string(),
            value: value.to_string(),
        })
    };

    if !in_xsl {
        // A literal result element: every attribute is an attribute value template.
        for (attribute, _, value) in &attrs {
            for expr in avt_expressions(value) {
                for construct in expression(&expr, at_root, true) {
                    report(construct, attribute, value);
                }
            }
        }
        return at_root;
    }

    let local = name.local_name().to_string();
    let get = |want: &str| {
        attrs
            .iter()
            .find(|(n, plain, _)| *plain && n == want)
            .map(|(_, _, v)| v.as_str())
    };
    // The context the element's OWN expressions are evaluated in, and its children's.
    let (own_at_root, children_at_root) = match local.as_str() {
        "template" => {
            let root = get("match").is_some_and(|m| m.trim() == "/");
            (root, root)
        }
        // `select` is evaluated where the for-each is; its body, once per selected node.
        "for-each" => (at_root, false),
        "sort" | "key" => (false, false),
        _ => (at_root, at_root),
    };
    for (attribute, _, value) in attrs.iter().filter(|(_, plain, _)| *plain) {
        match (local.as_str(), attribute.as_str()) {
            ("template" | "key", "match") => {
                if tokens(value).contains(&Tok::Open('[')) {
                    report(Construct::MatchPredicate, attribute, value);
                }
            }
            (_, "select" | "test" | "use" | "value") => {
                for construct in expression(value, own_at_root, false) {
                    report(construct, attribute, value);
                }
            }
            // The attribute value templates an XSL element takes.
            ("attribute" | "element", "name" | "namespace") => {
                for expr in avt_expressions(value) {
                    for construct in expression(&expr, own_at_root, false) {
                        report(construct, attribute, value);
                    }
                }
            }
            _ => {}
        }
    }
    children_at_root
}

/// The expressions inside an attribute value template's `{…}`, with `{{` and `}}` read as
/// the literal braces they are and a `}` inside a string literal left alone.
fn avt_expressions(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => {
                let mut expr = String::new();
                let mut quote: Option<char> = None;
                let mut closed = false;
                for c in chars.by_ref() {
                    match (quote, c) {
                        (Some(q), c) if c == q => quote = None,
                        (None, '\'' | '"') => quote = Some(c),
                        (None, '}') => {
                            closed = true;
                            break;
                        }
                        _ => {}
                    }
                    expr.push(c);
                }
                // An unterminated template is xrust's to refuse; there is nothing to read.
                if closed {
                    out.push(expr);
                }
            }
            _ => {}
        }
    }
    out
}

/// One XPath token. Only the distinctions the rules need are kept.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// A name: an NCName or a QName (`rdf:about`), or `prefix:*`.
    Name(String),
    /// `$name`.
    Variable,
    Number,
    Literal,
    /// `(` or `[`.
    Open(char),
    /// `)` or `]`.
    Close(char),
    /// `/`.
    Slash,
    /// `//`.
    SlashSlash,
    /// `*`, a wildcard or a multiplication.
    Star,
    /// `.` or `..`.
    Dot,
    /// `::`.
    Axis,
    /// `@`.
    At,
    /// `,`, `|`, `=`, `!=`, `<`, `<=`, `>`, `>=`, `+`, `-`.
    Operator,
    /// Anything else, which xrust will refuse or read its own way.
    Other,
}

/// Tokenize an XPath 1.0 expression or pattern.
fn tokens(expr: &str) -> Vec<Tok> {
    let chars: Vec<char> = expr.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let name_start =
        |c: char| c.is_alphabetic() || c == '_' || (!c.is_ascii() && !c.is_whitespace());
    let name_char =
        |c: char| name_start(c) || c.is_ascii_digit() || c == '-' || c == '.' || c == '\u{b7}';
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            c if c.is_whitespace() => i += 1,
            '\'' | '"' => {
                i += 1;
                while i < chars.len() && chars[i] != c {
                    i += 1;
                }
                i += 1;
                out.push(Tok::Literal);
            }
            '0'..='9' => {
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                out.push(Tok::Number);
            }
            '.' if next.is_some_and(|n| n.is_ascii_digit()) => {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(Tok::Number);
            }
            '.' => {
                i += if next == Some('.') { 2 } else { 1 };
                out.push(Tok::Dot);
            }
            '/' if next == Some('/') => {
                i += 2;
                out.push(Tok::SlashSlash);
            }
            '/' => {
                i += 1;
                out.push(Tok::Slash);
            }
            '(' | '[' => {
                i += 1;
                out.push(Tok::Open(c));
            }
            ')' | ']' => {
                i += 1;
                out.push(Tok::Close(c));
            }
            ':' if next == Some(':') => {
                i += 2;
                out.push(Tok::Axis);
            }
            '@' => {
                i += 1;
                out.push(Tok::At);
            }
            '*' => {
                i += 1;
                out.push(Tok::Star);
            }
            '$' => {
                i += 1;
                while i < chars.len() && (name_char(chars[i]) || chars[i] == ':') {
                    i += 1;
                }
                out.push(Tok::Variable);
            }
            '!' | '<' | '>' if next == Some('=') => {
                i += 2;
                out.push(Tok::Operator);
            }
            ',' | '|' | '=' | '<' | '>' | '+' | '-' => {
                i += 1;
                out.push(Tok::Operator);
            }
            c if name_start(c) => {
                let start = i;
                while i < chars.len() && name_char(chars[i]) {
                    i += 1;
                }
                // A QName's prefix: one `:` (not the axis separator `::`), then a name or `*`.
                if chars.get(i) == Some(&':') && chars.get(i + 1) != Some(&':') {
                    match chars.get(i + 1) {
                        Some('*') => i += 2,
                        Some(&n) if name_start(n) => {
                            i += 1;
                            while i < chars.len() && name_char(chars[i]) {
                                i += 1;
                            }
                        }
                        _ => {}
                    }
                }
                out.push(Tok::Name(chars[start..i].iter().collect()));
            }
            _ => {
                i += 1;
                out.push(Tok::Other);
            }
        }
    }
    out
}

/// Whether `tok`, coming after `prev`, ends an operand — XPath 1.0 §3.7's disambiguation: a
/// `*` or an operator name (`and`, `or`, `div`, `mod`) right after an operand is an operator,
/// and anything else is part of an operand.
fn ends_operand(prev_ends: bool, tok: &Tok) -> bool {
    match tok {
        Tok::Name(n) => !(prev_ends && matches!(n.as_str(), "and" | "or" | "div" | "mod")),
        Tok::Star => !prev_ends,
        Tok::Variable | Tok::Number | Tok::Literal | Tok::Close(_) | Tok::Dot => true,
        _ => false,
    }
}

/// The constructs one expression holds. `at_root`: the context node is the document root.
/// `avt`: it sits inside a literal result element's attribute value template.
fn expression(expr: &str, at_root: bool, avt: bool) -> Vec<Construct> {
    let toks = tokens(expr);
    let mut found = Vec::new();
    let mut add = |c: Construct| {
        if !found.contains(&c) {
            found.push(c)
        }
    };
    // The open brackets, innermost last.
    let mut open: Vec<char> = Vec::new();
    let mut prev_ends = false;
    for (i, tok) in toks.iter().enumerate() {
        let next = toks.get(i + 1);
        let in_predicate = open.contains(&'[');
        match tok {
            Tok::Open('[') => {
                let numeric = match (toks.get(i + 1), toks.get(i + 2), toks.get(i + 3)) {
                    (Some(Tok::Number), Some(Tok::Close(']')), _) => true,
                    (Some(Tok::Name(n)), Some(Tok::Open('(')), Some(Tok::Close(')'))) => {
                        n == "last" && toks.get(i + 4) == Some(&Tok::Close(']'))
                    }
                    _ => false,
                };
                if numeric {
                    add(Construct::NumericPredicate);
                }
            }
            Tok::Name(n) if next == Some(&Tok::Open('(')) => {
                let numeric_last = n == "last"
                    && i >= 1
                    && toks.get(i - 1) == Some(&Tok::Open('['))
                    && toks.get(i + 3) == Some(&Tok::Close(']'));
                match n.as_str() {
                    // `[last()]` is reported once, as the numeric predicate it is.
                    "last" if in_predicate && !numeric_last => add(Construct::PositionOrLast),
                    "position" | "last" if !in_predicate && !at_root => {
                        add(Construct::PositionOrLast)
                    }
                    _ => {}
                }
            }
            Tok::Name(n)
                if avt
                    && n.contains(':')
                    && next != Some(&Tok::Axis)
                    && ends_operand(prev_ends, tok) =>
            {
                add(Construct::PrefixedNameInAvt)
            }
            // Inside a predicate the context is the node being tested, never the root.
            Tok::SlashSlash if !prev_ends && (!at_root || in_predicate) => {
                add(Construct::LeadingDoubleSlash)
            }
            _ => {}
        }
        match tok {
            Tok::Open(c) => open.push(*c),
            Tok::Close(_) => {
                open.pop();
            }
            _ => {}
        }
        prev_ends = ends_operand(prev_ends, tok);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use Construct::*;

    fn at(expr: &str) -> Vec<Construct> {
        expression(expr, false, false)
    }

    #[test]
    fn a_numeric_predicate_is_seen_and_a_positional_test_is_not() {
        assert_eq!(at("doc/item[1]"), vec![NumericPredicate]);
        assert_eq!(at("count(doc/item[ 2 ])"), vec![NumericPredicate]);
        assert_eq!(at("(doc/item)[1]"), vec![NumericPredicate]);
        assert_eq!(at("doc/item[@k='1'][1]"), vec![NumericPredicate]);
        assert_eq!(at("doc/item[last()]"), vec![NumericPredicate]);
        assert_eq!(at("doc/item[position() = 1]"), vec![]);
        assert_eq!(at("doc/item[@k = 1]"), vec![]);
        assert_eq!(at("doc/item['1']"), vec![]);
        assert_eq!(at("concat('a[1]', \"b[2]\")"), vec![]);
    }

    #[test]
    fn position_and_last_are_refused_away_from_the_root() {
        assert_eq!(at("position()"), vec![PositionOrLast]);
        assert_eq!(at("position() = 2"), vec![PositionOrLast]);
        assert_eq!(at("last()"), vec![PositionOrLast]);
        assert_eq!(at("item[position() < last()]"), vec![PositionOrLast]);
        assert_eq!(at("item[last() - 1]"), vec![PositionOrLast]);
        assert_eq!(at("item[position() = 2]"), vec![]);
        assert_eq!(at("item[count(x[position() = 1]) = 1]"), vec![]);
        // At the root the context position and size really are 1.
        assert_eq!(expression("position()", true, false), vec![]);
        assert_eq!(expression("last()", true, false), vec![]);
        // `last()` inside a predicate is wrong at the root too.
        assert_eq!(
            expression("item[position() < last()]", true, false),
            vec![PositionOrLast]
        );
        // A name that only contains the word, and a string, are neither.
        assert_eq!(at("my-position() + l:last()"), vec![]);
        assert_eq!(at("'position()'"), vec![]);
    }

    #[test]
    fn only_a_leading_double_slash_is_refused_and_only_away_from_the_root() {
        assert_eq!(at("//item"), vec![LeadingDoubleSlash]);
        assert_eq!(at("count(//item)"), vec![LeadingDoubleSlash]);
        assert_eq!(at("a | //b"), vec![LeadingDoubleSlash]);
        assert_eq!(at("1 + //b"), vec![LeadingDoubleSlash]);
        assert_eq!(at("x and //b"), vec![LeadingDoubleSlash]);
        assert_eq!(at("x[//b]"), vec![LeadingDoubleSlash]);
        assert_eq!(at("doc//item"), vec![]);
        assert_eq!(at("/doc//item"), vec![]);
        assert_eq!(at(".//item"), vec![]);
        assert_eq!(at("*//item"), vec![]);
        assert_eq!(at("(a)//item"), vec![]);
        assert_eq!(at("a[1 = 1]//b"), vec![]);
        assert_eq!(at("'//item'"), vec![]);
        assert_eq!(at("concat(\"//\", '//')"), vec![]);
        assert_eq!(expression("//item", true, false), vec![]);
        // Inside a predicate the context is the node being tested, even at the root.
        assert_eq!(
            expression("doc/item[//item]", true, false),
            vec![LeadingDoubleSlash]
        );
    }

    #[test]
    fn a_prefixed_name_counts_only_in_a_literal_attribute_value_template() {
        let avt = |e: &str| expression(e, false, true);
        assert_eq!(avt("@rdf:about"), vec![PrefixedNameInAvt]);
        assert_eq!(avt("concat('x', @rdf:about)"), vec![PrefixedNameInAvt]);
        assert_eq!(avt("l:child"), vec![PrefixedNameInAvt]);
        assert_eq!(avt("l:*"), vec![PrefixedNameInAvt]);
        assert_eq!(avt("@id"), vec![]);
        assert_eq!(avt("child::item"), vec![]);
        assert_eq!(avt("'rdf:about'"), vec![]);
        assert_eq!(avt("$p:v"), vec![]);
        assert_eq!(avt("ext:fn(@id)"), vec![]);
        // Not in a literal result element's template, so not this construct.
        assert_eq!(at("@rdf:about"), vec![]);
    }

    #[test]
    fn attribute_value_templates_split_on_single_braces() {
        assert_eq!(avt_expressions("x-{@id}-{../@t}"), vec!["@id", "../@t"]);
        assert_eq!(avt_expressions("{{literal}} {'}'}"), vec!["'}'"]);
        assert_eq!(avt_expressions("no template"), Vec::<String>::new());
        assert_eq!(avt_expressions("{unterminated"), Vec::<String>::new());
    }
}
