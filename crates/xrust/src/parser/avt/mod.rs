/*!
Parse an Attribute Value Template.

See XSL Transformations v3.0 5.6.1.
 */

use crate::item::{Item, Node};
use crate::transform::Transform;
use crate::value::Value;
use crate::xdmerror::{Error, ErrorKind};
use std::rc::Rc;

/// Parse literal text and braced XPath, honoring doubled braces and quoted strings.
/// A [Node] is required to resolve in-scope XML Namespaces.
pub fn parse<N: Node>(input: &str, n: Option<N>) -> Result<Transform<N>, Error> {
    let invalid = |message: &str| Error::new(ErrorKind::ParseError, message);
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = input.char_indices().peekable();
    while let Some((offset, ch)) = chars.next() {
        match ch {
            '{' | '}' if chars.peek().is_some_and(|(_, next)| *next == ch) => {
                chars.next();
                literal.push(ch);
            }
            '}' => return Err(invalid("unescaped closing brace in attribute value template")),
            '{' => {
                if !literal.is_empty() {
                    parts.push(Transform::Literal(Item::Value(Rc::new(Value::from(
                        std::mem::take(&mut literal),
                    )))));
                }
                let mut quote = None;
                let mut end = None;
                for (index, ch) in chars.by_ref() {
                    match (quote, ch) {
                        (Some(delimiter), ch) if ch == delimiter => quote = None,
                        (None, '\'' | '"') => quote = Some(ch),
                        (None, '}') => { end = Some(index); break; }
                        (None, '{') => return Err(invalid("unquoted opening brace in attribute value template expression")),
                        _ => {}
                    }
                }
                let end = end.ok_or_else(|| invalid("no closing brace in attribute value template"))?;
                parts.push(crate::parser::xpath::parse(&input[offset + 1..end], n.clone(), None)?);
            }
            _ => literal.push(ch),
        }
    }
    if !literal.is_empty() || parts.is_empty() {
        parts.push(Transform::Literal(Item::Value(Rc::new(Value::from(literal)))));
    }
    Ok(if parts.len() == 1 { parts.pop().unwrap() } else { Transform::SequenceItems(parts) })
}
