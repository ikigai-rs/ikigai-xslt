//! XPath 1.0 numeric operands, adapted before xrust compiles them (#2487).
//!
//! xrust 2.3.0's Item::to_double rejected nodes and ValueData::UnsignedLong (the result of
//! string-length), and booleans had no string or number value. The xrust this crate carries
//! fixes those (ledger #3343). What is still adapted here: relational comparisons, which
//! XPath 1.0 makes numeric over every pair of items and xrust compares by the left operand's
//! type (lexically for a string), and unary signs, which xrust does not evaluate. Known
//! booleans and nodes are still converted explicitly, as before. Parse operator precedence
//! so the adaptation applies to operands, never to names, string literals or AVT text.
//!
//! Why the shared parser cannot serve here (xrust 2.3.0; ledger #2669):
//! `parser::xpath::parse` returns an editable `Transform`, and the standalone AST
//! test below proves that wrapping its arithmetic operands fixes conversion. But
//! `xslt::from_document` reparses attribute strings internally, accepts no XPath
//! compiler callback or pre-parsed expressions, and returns a `Context` whose
//! template bodies, globals and keys are crate-private. We cannot install those
//! edited expressions into the stylesheet compiled by our existing XSLT pipeline.
//! Unlike boolean_values, this adapter must change operands inside expressions,
//! not just classify a result and emit ordinary XSLT around the original string.
//!
//! Nor does `Transform` retain source spans or lexical spellings: arithmetic has
//! only operator/operand fields, literals become values, and QName resolution can
//! discard prefixes. Its Debug output is diagnostic (e.g. "Arithmetic 2 operands"),
//! not XPath serialization. Locating nested operands in the original text would
//! still require parsing; writing an AST-to-XPath serializer would add another
//! substantial syntax implementation. Retain this bounded source adapter until
//! upstream exposes a stylesheet XPath compilation hook, a mutable compiled-tree
//! visitor, or operand spans. See docs/numeric-cost.md for API evidence, the AST
//! experiment, and chart-sized wrapper costs beside PR 39's RSS measurements.

use crate::{
    boolean_values::{self, Kind},
    node_id::IdNode,
    XSL_NS,
};
use ikigai_core::{Error, Result};
use std::rc::Rc;
use xrust::{item::Node, value::Value};

// Each token retains its source spelling. Names include hyphens and QNames;
// an asterisk is interpreted as multiplication only after a complete operand.
// Also used to recognize namespaced calls before boolean type analysis: xrust
// discards their namespaces when their local names match core functions.
pub(crate) fn tokens(s: &str) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let start = i;
        let c = s[i..].chars().next()?;
        i += c.len_utf8();
        if c.is_whitespace() {
            continue;
        }
        if c == '\'' || c == '"' {
            let end = s[i..].find(c)?;
            i += end + c.len_utf8();
        } else if c.is_ascii_digit()
            || (c == '.' && s[i..].starts_with(|c: char| c.is_ascii_digit()))
        {
            while i < s.len() && (s.as_bytes()[i].is_ascii_digit() || s.as_bytes()[i] == b'.') {
                i += 1;
            }
        } else if c.is_alphabetic() || c == '_' {
            while let Some(c) = s[i..].chars().next() {
                if c.is_alphanumeric() || c >= '\u{80}' || matches!(c, '_' | '-' | '.') {
                    i += c.len_utf8();
                } else if c == ':' && !s[i..].starts_with("::") {
                    i += 1;
                } else {
                    break;
                }
            }
        } else if ["//", "..", "::", "!=", "<=", ">="]
            .iter()
            .any(|pair| s[start..].starts_with(pair))
        {
            i += 1;
        }
        out.push(&s[start..i]);
    }
    Some(out)
}

struct Expr {
    text: String,
    kind: Kind,
    is_string: bool,
}

impl Expr {
    fn other(text: String) -> Self {
        Self {
            text,
            kind: Kind::Other,
            is_string: false,
        }
    }

    fn string(text: String) -> Self {
        Self {
            text,
            kind: Kind::Other,
            is_string: true,
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

struct Parser<'a> {
    tokens: Vec<&'a str>,
    pos: usize,
    changed: bool,
    depth: usize,
    node: Option<IdNode>,
    item_name: String,
}

fn numeric(value: &str) -> String {
    // [1] matters: XPath 1.0 converts the first node, not the concatenation of all
    // nodes that xrust's string() otherwise produces. Empty selections become NaN.
    format!("normalize-space(string(({value})[1]))")
}

impl Parser<'_> {
    fn conversion(&mut self, value: &Expr, string: bool) -> String {
        // A string literal or a core string function already returns one string.
        // Rewrapping nested scalar calls grows the XPath fed back to xrust without
        // changing its value. Node selections and unknown values still need [1].
        if string && value.is_string {
            return value.text.clone();
        }
        match value.kind {
            Kind::Boolean => {
                // xrust supports conditional expressions internally. Each branch
                // has the required type and the condition is evaluated once.
                if string {
                    format!("(if ({value}) then 'true' else 'false')")
                } else {
                    format!("(if ({value}) then 1 else 0)")
                }
            }
            Kind::Other => {
                if string {
                    format!("string(({value})[1])")
                } else {
                    numeric(&value.text)
                }
            }
            // A value that may be a boolean or not is left to xrust, which converts both
            // since the carried xrust's boolean and node conversions (ledger #3343).
            Kind::Unknown => {
                if string {
                    format!("string(({value})[1])")
                } else {
                    value.text.clone()
                }
            }
        }
    }

    fn numeric_items(&mut self, value: &Expr) -> String {
        // Relational comparisons are existential over both node-sets, unlike
        // number(node-set), which takes only the first node. Atomize each item
        // before comparing: xrust reverses the operands when the left is a node.
        let item = Expr {
            text: format!("${}", self.item_name),
            kind: value.kind,
            is_string: false,
        };
        let converted = self.conversion(&item, false);
        format!(
            "(for ${} in ({value}) return number({converted}))",
            self.item_name
        )
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).copied()
    }

    fn take(&mut self) -> Option<&str> {
        let token = self.tokens.get(self.pos).copied();
        self.pos += usize::from(token.is_some());
        token
    }

    fn expect(&mut self, want: &str) -> Option<()> {
        (self.take()? == want).then_some(())
    }

    // XPath 1.0 precedence, low to high. Path and predicate parsing is in primary.
    fn expression(&mut self, min: u8) -> Option<Expr> {
        self.bounded(|parser| parser.expression_inner(min))
    }

    fn bounded(&mut self, parse: impl FnOnce(&mut Self) -> Option<Expr>) -> Option<Expr> {
        // Brackets are bounded in caller input, but unary signs and malformed axis
        // chains can recurse without brackets. Leave such syntax to xrust.
        if self.depth >= 4 * crate::limits::MAX_EXPR_DEPTH {
            return None;
        }
        self.depth += 1;
        let result = parse(self);
        self.depth -= 1;
        result
    }

    fn expression_inner(&mut self, min: u8) -> Option<Expr> {
        if min == 1 && self.peek() == Some("if") && self.tokens.get(self.pos + 1) == Some(&"(") {
            self.take();
            self.expect("(")?;
            let condition = self.expression(1)?;
            self.expect(")")?;
            self.expect("then")?;
            let yes = self.expression(1)?;
            self.expect("else")?;
            let no = self.expression(1)?;
            return Some(Expr {
                text: format!("(if ({condition}) then {yes} else {no})"),
                is_string: yes.is_string && no.is_string,
                kind: if yes.kind == no.kind {
                    yes.kind
                } else {
                    Kind::Unknown
                },
            });
        }
        let mut left = if matches!(self.peek(), Some("-" | "+")) {
            let sign = self.take()?.to_string();
            let value = self.expression(7)?;
            self.changed = true;
            // xrust's unary expression evaluator is unimplemented. Multiplication
            // applies the sign without losing negative zero (unlike 0 - value).
            let factor = if sign == "-" { "number('-1')" } else { "1" };
            Expr::other(format!("({} * {factor})", self.conversion(&value, false)))
        } else {
            self.primary()?
        };
        while let Some(op) = self.peek() {
            let precedence = match op {
                "or" => 1,
                "and" => 2,
                "=" | "!=" => 3,
                "<" | ">" | "<=" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "div" | "mod" => 6,
                "|" => 8,
                "/" | "//" => 9,
                _ => break,
            };
            if precedence < min {
                break;
            }
            let op = self.take()?.to_string();
            let right = self.expression(precedence + 1)?;
            left = if matches!(precedence, 5 | 6) {
                self.changed = true;
                Expr::other(format!(
                    "({} {op} {})",
                    self.conversion(&left, false),
                    self.conversion(&right, false)
                ))
            } else if precedence == 4 {
                self.changed = true;
                Expr {
                    text: format!(
                        "({} {op} {})",
                        self.numeric_items(&left),
                        self.numeric_items(&right)
                    ),
                    kind: Kind::Boolean,
                    is_string: false,
                }
            } else {
                Expr {
                    text: format!("{left} {op} {right}"),
                    is_string: false,
                    kind: if precedence <= 4 {
                        Kind::Boolean
                    } else {
                        Kind::Other
                    },
                }
            };
        }
        Some(left)
    }

    fn primary(&mut self) -> Option<Expr> {
        self.bounded(Self::primary_inner)
    }

    fn primary_inner(&mut self) -> Option<Expr> {
        let token = self.take()?.to_string();
        let mut value = match token.as_str() {
            "(" => {
                let value = self.expression(1)?;
                self.expect(")")?;
                Expr {
                    text: format!("({value})"),
                    kind: value.kind,
                    is_string: value.is_string,
                }
            }
            "/" | "//" => {
                if matches!(
                    self.peek(),
                    None | Some(
                        ")" | "]" | "," | "|" | "+" | "-" | "=" | "!=" | "<" | ">" | "<=" | ">="
                    )
                ) {
                    Expr::other(token)
                } else {
                    Expr::other(format!("{token}{}", self.expression(10)?))
                }
            }
            "@" | "$" => {
                let name = self.take()?.to_string();
                if token == "@" && name.ends_with(':') && self.peek() == Some("*") {
                    self.take();
                    Expr::other(format!("@{name}*"))
                } else {
                    let text = format!("{token}{name}");
                    let kind = if token == "$" {
                        self.node
                            .as_ref()
                            .map(|n| boolean_values::expression_kind(&text, n))
                            .unwrap_or(Kind::Unknown)
                    } else {
                        Kind::Other
                    };
                    Expr {
                        text,
                        kind,
                        is_string: false,
                    }
                }
            }
            ")" | "]" | "," | "=" | "!=" | "<" | ">" | "<=" | ">=" => return None,
            _ => {
                if self.peek() == Some("(") {
                    self.take();
                    let mut args = Vec::new();
                    if self.peek() != Some(")") {
                        loop {
                            args.push(self.expression(1)?);
                            if self.peek() != Some(",") {
                                break;
                            }
                            self.take();
                        }
                    }
                    self.expect(")")?;
                    if matches!(token.as_str(), "number" | "string") && args.len() <= 1 {
                        self.changed = true;
                        let default = Expr::other(".".into());
                        let arg = args.first().unwrap_or(&default);
                        let converted = self.conversion(arg, token == "string");
                        if token == "number" {
                            Expr::other(format!("(number({converted}) * 1.0)"))
                        } else {
                            Expr::string(converted)
                        }
                    } else {
                        // XPath 1.0 string arguments use the first node's string value.
                        // xrust's later-version sequence conversion concatenates them.
                        let string_args = match token.as_str() {
                            "concat" | "contains" | "starts-with" | "substring-before"
                            | "substring-after" | "translate" => args.len(),
                            "string-length" | "normalize-space" | "substring" | "lang" => {
                                args.len().min(1)
                            }
                            _ => 0,
                        };
                        for arg in args.iter_mut().take(string_args) {
                            let text = self.conversion(arg, true);
                            *arg = Expr::string(text);
                            self.changed = true;
                        }
                        let text = format!(
                            "{token}({})",
                            args.iter()
                                .map(|a| a.text.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        // Core scalar result types do not depend on their arguments.
                        // Parsing the expanded arguments here repeats all generated
                        // first-node conversions, once for every enclosing call.
                        let kind = match token.as_str() {
                            "true" | "false" | "boolean" | "not" | "contains" | "starts-with"
                            | "lang" => Kind::Boolean,
                            "concat" | "substring" | "substring-before" | "substring-after"
                            | "normalize-space" | "translate" | "string-length" | "count"
                            | "sum" | "floor" | "ceiling" | "round" | "position" | "last"
                            | "name" | "local-name" | "namespace-uri" => Kind::Other,
                            // Keep the existing conservative analysis for other calls,
                            // including namespaced spellings of core functions.
                            _ => match &self.node {
                                Some(n) => boolean_values::expression_kind(&text, n),
                                None => Kind::Other,
                            },
                        };
                        let is_string = matches!(
                            token.as_str(),
                            "concat"
                                | "substring"
                                | "substring-before"
                                | "substring-after"
                                | "normalize-space"
                                | "translate"
                                | "name"
                                | "local-name"
                                | "namespace-uri"
                        );
                        Expr {
                            text,
                            kind,
                            is_string,
                        }
                    }
                } else if self.peek() == Some("::") {
                    self.take();
                    Expr::other(format!("{token}::{}", self.primary()?))
                } else if token.ends_with(':') && self.peek() == Some("*") {
                    self.take();
                    Expr::other(format!("{token}*"))
                } else if token.bytes().all(|c| c.is_ascii_digit())
                    && token
                        .parse::<u64>()
                        .map_or(true, |value| value >= (1_u64 << 53))
                {
                    // XPath 1.0 literals are doubles. xrust's exact integers can
                    // distinguish adjacent integers above the double precision limit.
                    self.changed = true;
                    Expr::other(format!("{token}.0"))
                } else if token.starts_with(['\'', '"']) {
                    Expr::string(token)
                } else {
                    Expr::other(token)
                }
            }
        };
        while self.peek() == Some("[") {
            self.take();
            let predicate = self.expression(1)?;
            self.expect("]")?;
            value = Expr {
                text: format!("{value}[{predicate}]"),
                is_string: false,
                kind: if value.kind == Kind::Other {
                    Kind::Other
                } else {
                    Kind::Unknown
                },
            };
        }
        Some(value)
    }
}

fn expression_at(source: &str, node: Option<&IdNode>) -> Result<Option<String>> {
    let mut item_name = "__ikigai_numeric_item".to_string();
    while source.contains(&item_name) {
        item_name.push('_');
    }
    let mut parser = Parser {
        tokens: match tokens(source) {
            Some(t) => t,
            None => return Ok(None),
        },
        pos: 0,
        changed: false,
        depth: 0,
        node: node.cloned(),
        item_name,
    };
    let result = parser.expression(1);
    if result.is_none() || parser.pos != parser.tokens.len() {
        // xrust also accepts syntax outside this source adapter's grammar (for
        // example XPath 2.0 for/let bindings). Leaving a conversion in that syntax
        // untouched would bypass the comparison and sign adaptations. Refuse valid expressions that
        // might need adaptation; malformed XPath remains the compiler's error.
        if node.is_some()
            && parser.tokens.iter().any(|t| {
                matches!(
                    *t,
                    "number"
                        | "string"
                        | "concat"
                        | "contains"
                        | "starts-with"
                        | "substring-before"
                        | "substring-after"
                        | "translate"
                        | "string-length"
                        | "normalize-space"
                        | "substring"
                        | "lang"
                        | "+"
                        | "-"
                        | "*"
                        | "div"
                        | "mod"
                        | "<"
                        | ">"
                        | "<="
                        | ">="
                )
            })
            && xrust::parser::xpath::parse(source, node.cloned(), None).is_ok()
        {
            return Err(Error::InvalidArgument {
                name: "stylesheet".into(),
                detail: format!("refused: use xsl:choose to convert values explicitly. Numeric/string conversion in `{source}` uses syntax whose operand types cannot be established"),
            });
        }
        return Ok(None);
    }
    let result = result.unwrap();
    Ok(parser.changed.then_some(result.text))
}

fn avt_at(source: &str, node: Option<&IdNode>) -> Result<String> {
    let mut result = String::new();
    let mut i = 0;
    while i < source.len() {
        if source[i..].starts_with("{{") || source[i..].starts_with("}}") {
            result.push_str(&source[i..i + 2]);
            i += 2;
        } else if source.as_bytes()[i] == b'{' {
            let start = i + 1;
            let mut quote = None;
            i = start;
            while i < source.len() {
                let c = source[i..].chars().next().unwrap();
                match (quote, c) {
                    (Some(q), c) if c == q => quote = None,
                    (None, '\'' | '"') => quote = Some(c),
                    (None, '}') => break,
                    _ => {}
                }
                i += c.len_utf8();
            }
            if i == source.len() {
                result.push_str(&source[start - 1..]);
                break;
            }
            let original = &source[start..i];
            let adapted = expression_at(original, node)?.unwrap_or_else(|| original.to_string());
            result.push('{');
            result.push_str(&format!("string(({adapted})[1])"));
            result.push('}');
            i += 1;
        } else {
            let c = source[i..].chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    Ok(result)
}

#[cfg(test)]
fn expression(source: &str) -> Option<String> {
    expression_at(source, None).unwrap()
}

#[cfg(test)]
fn avt(source: &str) -> String {
    avt_at(source, None).unwrap()
}

pub(crate) fn prepare(doc: &IdNode) -> Result<()> {
    let mut stack = vec![doc.clone()];
    while let Some(node) = stack.pop() {
        stack.extend(node.child_iter());
        let Some(name) = node.name() else { continue };
        let in_xsl = name
            .namespace_uri()
            .is_some_and(|ns| ns.to_string() == XSL_NS);
        let local = name.local_name().to_string();
        for mut attr in node.attribute_iter().collect::<Vec<_>>() {
            let name = attr.name().unwrap();
            if name
                .namespace_uri()
                .is_some_and(|ns| in_xsl || ns.to_string() == XSL_NS)
            {
                continue;
            }
            let source = attr.value().to_string();
            let attr_local = name.local_name().to_string();
            let replacement = if !in_xsl
                || (matches!(
                    local.as_str(),
                    "attribute" | "element" | "processing-instruction"
                ) && matches!(attr_local.as_str(), "name" | "namespace"))
            {
                Some(avt_at(&source, Some(&node))?)
            } else if matches!(attr_local.as_str(), "select" | "test" | "use" | "value") {
                // XSLT 1.0 value-of converts the first node, not the whole sequence.
                // Keep this compatibility rule here rather than changing xrust's
                // sequence model for its XPath/XSLT 3.0 consumers.
                if local == "value-of" && attr_local == "select" {
                    let adapted = expression_at(&source, Some(&node))?.unwrap_or(source.clone());
                    Some(format!("string(({adapted})[1])"))
                } else {
                    expression_at(&source, Some(&node))?
                }
            } else {
                None
            };
            if let Some(value) = replacement.filter(|value| value != &source) {
                // Rewriting inserts parentheses. Enforce the same parser-stack bound on
                // the generated expression as on caller input, before xrust sees it.
                let escaped = value
                    .replace('&', "&amp;")
                    .replace('"', "&quot;")
                    .replace('<', "&lt;");
                crate::limits::check_stylesheet(&format!("<e a=\"{escaped}\"/>"), "stylesheet")?;
                attr.pop().map_err(|e| Error::Endpoint(e.message))?;
                let replacement = node
                    .new_attribute(name, Rc::new(Value::from(value)))
                    .map_err(|e| Error::Endpoint(e.message))?;
                node.add_attribute(replacement)
                    .map_err(|e| Error::Endpoint(e.message))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The standalone AST is editable. The missing piece is handing that edited
    // AST to the stylesheet compiler; see the module header and docs/numeric-cost.md.
    #[test]
    fn standalone_xrust_ast_can_adapt_numeric_operands() {
        use xrust::{
            item::{Item, SequenceTrait},
            parser::xpath::parse,
            transform::{
                context::{Context, StaticContextBuilder},
                Transform,
            },
        };

        let doc = crate::parse_xml("<r><v> 12.5 </v><v>99</v></r>").unwrap();
        let evaluate = |t: &Transform<IdNode>| {
            let mut context = Context::new();
            context.context(vec![Item::Node(doc.clone())], 0);
            let mut static_context = StaticContextBuilder::new()
                .message(|_| Ok(()))
                .fetcher(|_| Ok(String::new()))
                .parser(|_| Ok(doc.clone()))
                .build();
            context
                .dispatch(&mut static_context, t)
                .unwrap()
                .to_string()
        };
        let mut t = parse::<IdNode>("/r/v + 1", None, None).unwrap();
        assert_eq!(evaluate(&t), "13.5");
        let Transform::Arithmetic(operands) = &mut t else {
            panic!("expected arithmetic AST");
        };
        for operand in operands {
            let first = Transform::Compose(vec![
                operand.operand.clone(),
                Transform::Filter(Box::new(parse::<IdNode>("1", None, None).unwrap())),
            ]);
            operand.operand =
                Transform::NormalizeSpace(Some(Box::new(Transform::String(Box::new(first)))));
        }
        assert_eq!(evaluate(&t), "13.5");
    }

    #[test]
    fn avt_text_escaped_braces_and_quoted_braces_are_preserved() {
        let source = "é + 1 {{number(/r)}} {concat('}', /r + 1)} {{x}}";
        let stylesheet = format!(
            r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="1.0"><xsl:template match="/"><e a="{source}"/></xsl:template></xsl:stylesheet>"#
        );
        assert_eq!(
            crate::transform_xml("<r>2</r>", &stylesheet, false).unwrap(),
            "<e a='é + 1 {number(/r)} }3 {x}'/>"
        );
        assert_eq!(avt("{number(/r)"), "{number(/r)");
    }

    #[test]
    fn non_numeric_and_unsupported_syntax_is_left_to_xrust() {
        for source in [
            "/r/a-b",
            "'number(/r) + 1'",
            "/r[number(@n) + 1",
            "if (true()) then 1 else 0",
            "concat('unterminated",
            "(é)",
        ] {
            assert_eq!(expression(source), None, "{source}");
        }
    }

    #[test]
    fn unbracketed_recursion_is_bounded() {
        for source in [
            format!("{}1", "- ".repeat(1000)),
            format!("{}v + 1", "child::".repeat(1000)),
        ] {
            assert_eq!(expression(&source), None);
        }
    }

    #[test]
    fn expanded_expression_depth_is_checked_before_compilation() {
        let source = format!(
            "number({}1{})",
            "(".repeat(crate::limits::MAX_EXPR_DEPTH - 2),
            ")".repeat(crate::limits::MAX_EXPR_DEPTH - 2)
        );
        let stylesheet = format!(
            r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template match="/"><xsl:value-of select="{source}"/></xsl:template></xsl:stylesheet>"#
        );
        let error = crate::transform_xml("<r/>", &stylesheet, true).unwrap_err();
        assert!(
            error.starts_with("invalid argument `stylesheet`:"),
            "{error}"
        );
        assert!(error.contains("MAX_EXPR_DEPTH"), "{error}");
    }
}
