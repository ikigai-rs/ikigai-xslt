//! Refuse output features the XML serializer cannot honor.
use crate::{node_id::IdNode, XSL_NS};
use ikigai_core::{Error, Result};
use xrust::item::Node;

pub(crate) fn invalid(detail: impl Into<String>) -> Error {
    Error::InvalidArgument {
        name: "stylesheet".into(),
        detail: detail.into(),
    }
}

pub(crate) fn check(doc: &IdNode) -> Result<()> {
    let mut stack = vec![doc.clone()];
    while let Some(node) = stack.pop() {
        stack.extend(node.child_iter());
        let Some(name) = node.name() else { continue };
        let in_xsl = name
            .namespace_uri()
            .is_some_and(|ns| ns.to_string() == XSL_NS);
        let local = name.local_name().to_string();
        if in_xsl && local == "namespace-alias" {
            return Err(invalid("xsl:namespace-alias is not supported; write literal result names in the result namespace"));
        }
        for attr in node.attribute_iter() {
            let name = attr.name().unwrap();
            if name
                .namespace_uri()
                .is_some_and(|ns| in_xsl || ns.to_string() == XSL_NS)
            {
                continue;
            }
            let attribute = name.local_name().to_string();
            let value = attr.value().to_string();
            if in_xsl && local == "output" {
                if attribute == "encoding" && !value.eq_ignore_ascii_case("UTF-8") {
                    return Err(invalid(format!(
                        "xsl:output encoding `{value}` is not supported; use UTF-8"
                    )));
                }
                if attribute == "omit-xml-declaration" && value != "yes" {
                    return Err(invalid("xsl:output omit-xml-declaration must be yes; this serializer emits XML fragments without a declaration"));
                }
            }
            let avt = !in_xsl
                || (matches!(
                    local.as_str(),
                    "element" | "attribute" | "processing-instruction"
                ) && matches!(attribute.as_str(), "name" | "namespace"));
            if avt && escaped_brace(&value) {
                return Err(invalid(format!("escaped braces ({{{{ or }}}}) in an attribute value template on `{local}` are not supported; use xsl:attribute with xsl:text for literal braces")));
            }
        }
    }
    Ok(())
}

// Doubled braces inside a quoted XPath string are ordinary string characters.
fn escaped_brace(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    let mut expression = false;
    let mut quote = None;
    while let Some(c) = chars.next() {
        if expression {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (None, '\'' | '"') => quote = Some(c),
                (None, '}') => expression = false,
                _ => {}
            }
        } else if matches!(c, '{' | '}') && chars.peek() == Some(&c) {
            return true;
        } else if c == '{' {
            expression = true;
        }
    }
    false
}
