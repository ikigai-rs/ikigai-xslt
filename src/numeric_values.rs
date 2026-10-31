//! XPath 1.0 numeric operands, adapted before xrust compiles them (#2487).
//!
//! xrust's Item::to_double rejects nodes and ValueData::UnsignedLong (the result of
//! string-length), and booleans have no string value upstream. Convert known
//! booleans before stringification; reject ambiguous conversions. Its
//! number(string(...)) path works for nodes. Parse operator precedence so
//! the adaptation applies to operands, never to names, string literals or AVT text.

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

struct Expr {
    text: String,
    kind: Kind,
}

impl Expr {
    fn other(text: String) -> Self {
        Self {
            text,
            kind: Kind::Other,
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
    ambiguous: bool,
    item_name: String,
}

fn numeric(value: &str) -> String {
    // [1] matters: XPath 1.0 converts the first node, not the concatenation of all
    // nodes that xrust's string() otherwise produces. Empty selections become NaN.
    format!("normalize-space(string(({value})[1]))")
}

impl Parser<'_> {
    fn conversion(&mut self, value: &Expr, string: bool) -> String {
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
                    format!("string({value})")
                } else {
                    numeric(&value.text)
                }
            }
            Kind::Unknown => {
                self.ambiguous = true;
                value.text.clone()
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
                }
            } else {
                Expr {
                    text: format!("{left} {op} {right}"),
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
                    Expr { text, kind }
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
                        Expr::other(if token == "number" {
                            format!("number({converted})")
                        } else {
                            converted
                        })
                    } else {
                        let kind = if matches!(
                            token.as_str(),
                            "true"
                                | "false"
                                | "boolean"
                                | "not"
                                | "contains"
                                | "starts-with"
                                | "ends-with"
                        ) {
                            Kind::Boolean
                        } else {
                            Kind::Other
                        };
                        Expr {
                            text: format!(
                                "{token}({})",
                                args.iter()
                                    .map(|a| a.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            kind,
                        }
                    }
                } else if self.peek() == Some("::") {
                    self.take();
                    Expr::other(format!("{token}::{}", self.primary()?))
                } else if token.ends_with(':') && self.peek() == Some("*") {
                    self.take();
                    Expr::other(format!("{token}*"))
                } else if token.bytes().all(|c| c.is_ascii_digit()) && token.parse::<i64>().is_err()
                {
                    // XPath 1.0 literals are doubles; xrust first tries an i64.
                    self.changed = true;
                    Expr::other(format!("number('{token}')"))
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
        ambiguous: false,
        item_name,
    };
    let Some(result) = parser.expression(1) else {
        return Ok(None);
    };
    if parser.pos != parser.tokens.len() {
        return Ok(None);
    }
    if parser.ambiguous {
        return Err(Error::InvalidArgument {
            name: "stylesheet".into(),
            detail: format!("refused: numeric/string conversion in `{source}` may receive a boolean or a nonboolean value (ledger #2668); use xsl:choose to convert mixed parameter values explicitly"),
        });
    }
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
            result.push('{');
            result.push_str(
                expression_at(original, node)?
                    .as_deref()
                    .unwrap_or(original),
            );
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
                expression_at(&source, Some(&node))?
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
