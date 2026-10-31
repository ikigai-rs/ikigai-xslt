//! XPath 1.0 numeric operands, adapted before xrust compiles them (#2487).
//!
//! xrust's Item::to_double rejects nodes and ValueData::UnsignedLong (the result of
//! string-length). Its number(string(...)) path works. Parse operator precedence so
//! the adaptation applies to operands, never to names, string literals or AVT text.
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

use crate::{node_id::IdNode, XSL_NS};
use ikigai_core::{Error, Result};
use std::rc::Rc;
use xrust::{item::Node, value::Value};

// Each token retains its source spelling. Names include hyphens and QNames;
// an asterisk is interpreted as multiplication only after a complete operand.
fn tokens(s: &str) -> Option<Vec<&str>> {
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

struct Parser<'a> {
    tokens: Vec<&'a str>,
    pos: usize,
    changed: bool,
    depth: usize,
}

fn numeric(value: &str) -> String {
    // [1] matters: XPath 1.0 converts the first node, not the concatenation of all
    // nodes that xrust's string() otherwise produces. Empty selections become NaN.
    format!("normalize-space(string(({value})[1]))")
}

impl Parser<'_> {
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
    fn expression(&mut self, min: u8) -> Option<String> {
        self.bounded(|parser| parser.expression_inner(min))
    }

    fn bounded(&mut self, parse: impl FnOnce(&mut Self) -> Option<String>) -> Option<String> {
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

    fn expression_inner(&mut self, min: u8) -> Option<String> {
        let mut left = if matches!(self.peek(), Some("-" | "+")) {
            let sign = self.take()?.to_string();
            let value = self.expression(7)?;
            format!("{sign}{value}")
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
                format!("({} {op} {})", numeric(&left), numeric(&right))
            } else {
                format!("{left} {op} {right}")
            };
        }
        Some(left)
    }

    fn primary(&mut self) -> Option<String> {
        self.bounded(Self::primary_inner)
    }

    fn primary_inner(&mut self) -> Option<String> {
        let token = self.take()?.to_string();
        let mut value = match token.as_str() {
            "(" => {
                let value = self.expression(1)?;
                self.expect(")")?;
                format!("({value})")
            }
            "/" | "//" => {
                if matches!(
                    self.peek(),
                    None | Some(
                        ")" | "]" | "," | "|" | "+" | "-" | "=" | "!=" | "<" | ">" | "<=" | ">="
                    )
                ) {
                    token
                } else {
                    format!("{token}{}", self.expression(10)?)
                }
            }
            "@" | "$" => {
                let name = self.take()?.to_string();
                if token == "@" && name.ends_with(':') && self.peek() == Some("*") {
                    self.take();
                    format!("@{name}*")
                } else {
                    format!("{token}{name}")
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
                    if token == "number" && args.len() <= 1 {
                        self.changed = true;
                        format!(
                            "number({})",
                            numeric(args.first().map(String::as_str).unwrap_or("."))
                        )
                    } else {
                        format!("{token}({})", args.join(", "))
                    }
                } else if self.peek() == Some("::") {
                    self.take();
                    format!("{token}::{}", self.primary()?)
                } else if token.ends_with(':') && self.peek() == Some("*") {
                    self.take();
                    format!("{token}*")
                } else {
                    token
                }
            }
        };
        while self.peek() == Some("[") {
            self.take();
            let predicate = self.expression(1)?;
            self.expect("]")?;
            value = format!("{value}[{predicate}]");
        }
        Some(value)
    }
}

fn expression(source: &str) -> Option<String> {
    let mut parser = Parser {
        tokens: tokens(source)?,
        pos: 0,
        changed: false,
        depth: 0,
    };
    let result = parser.expression(1)?;
    (parser.changed && parser.pos == parser.tokens.len()).then_some(result)
}

fn avt(source: &str) -> String {
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
            result.push('{');
            result.push_str(expression(original).as_deref().unwrap_or(original));
            result.push('}');
            i += 1;
        } else {
            let c = source[i..].chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    result
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
                Some(avt(&source))
            } else if matches!(attr_local.as_str(), "select" | "test" | "use" | "value") {
                expression(&source)
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
        assert_eq!(evaluate(&t), "NaN");
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
        let expected = format!(
            "é + 1 {{{{number(/r)}}}} {{concat('}}', ({} + {}))}} {{{{x}}}}",
            numeric("/r"),
            numeric("1")
        );
        assert_eq!(avt(source), expected);
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
