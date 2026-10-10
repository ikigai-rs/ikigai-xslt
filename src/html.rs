//! `xsl:output method="html"`: the result serialized as HTML, not XML (ledger #193).
//!
//! xrust 2.2.0 has one serializer, XML's, so every empty element comes out self-closed, and a
//! browser reads `<script src='…'/>` as an OPEN script element that swallows the rest of the
//! page (`<textarea/>` and `<div/>` misnest the same way). XSLT 1.0 §16.2 says the html method
//! writes no end tag for an empty element only when HTML calls it void, and an end tag for
//! every other.
//!
//! So before xrust serializes a result for the html method, [`open_empty_elements`] gives each
//! empty, non-void element in no namespace an empty text child, and xrust's own serializer then
//! writes `<script src='…'></script>`. Everything else — escaping, attribute quoting, namespace
//! declarations, comments — is byte for byte what xrust writes, so the change is exactly the
//! empty-element case:
//!
//! - an empty **non-void** element in no namespace keeps its end tag: `<div></div>`;
//! - a **void** element (HTML's list, compared ignoring case) has none: `<br/>`. The trailing
//!   slash is xrust's, and HTML ignores it on a void element; it is what gonk's own pass wrote,
//!   so a consumer dropping that pass sees no change;
//! - an element **in a namespace** (inline SVG, MathML) is XML, as §16.2 says, and keeps
//!   `<path/>`, which an HTML parser reads correctly inside foreign content.
//!
//! ⚠ Not done: §16.2 also writes the text of `script` and `style` unescaped, and xrust escapes
//! it (`'` becomes `&apos;`, `<` becomes `&lt;`), so an INLINE script or style with a quote or a
//! `<` in it is broken. Writing it raw would make any data a stylesheet copies into a script
//! element executable markup, so that is a decision for a host, not a serializer default: keep
//! scripts and styles in their own files (`<script src='…'>`).

use std::rc::Rc;

use xrust::item::{Item, Node, NodeType, Sequence};
use xrust::value::Value;

use crate::node_id::IdNode;

/// The elements HTML serializes with no end tag (the WHATWG serializing algorithm's list,
/// which keeps the legacy ones an HTML parser still treats as void).
const VOID: [&str; 18] = [
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// Whether a result serializes as HTML: `method="html"`, or — XSLT 1.0 §16's default when the
/// stylesheet names no method — a result whose first element is `html` in no namespace, with
/// only whitespace before it.
pub(crate) fn is_html(method: Option<&str>, seq: &Sequence<IdNode>) -> bool {
    match method {
        Some(method) => method == "html",
        None => first_element_is_html(seq),
    }
}

fn first_element_is_html(seq: &Sequence<IdNode>) -> bool {
    // The result's top-level nodes, a document's children standing in for the document.
    let mut top: Vec<IdNode> = Vec::new();
    for item in seq {
        match item {
            Item::Node(n) if n.node_type() == NodeType::Document => top.extend(n.child_iter()),
            Item::Node(n) => top.push(n.clone()),
            Item::Value(v) if v.to_string().trim().is_empty() => {}
            _ => return false,
        }
    }
    for node in top {
        match node.node_type() {
            NodeType::Element => {
                return node.name().is_some_and(|qn| {
                    qn.namespace_uri().is_none()
                        && qn.local_name().to_string().eq_ignore_ascii_case("html")
                })
            }
            NodeType::Text if node.to_string().trim().is_empty() => {}
            NodeType::Comment | NodeType::ProcessingInstruction => {}
            _ => return false,
        }
    }
    false
}

/// Give every empty, non-void element in no namespace under `seq` an empty text child, so
/// that xrust's serializer writes its end tag. Walks with an explicit stack: a result can nest
/// far deeper than a stylesheet (README, "Caller XML is bounded").
pub(crate) fn open_empty_elements(seq: &Sequence<IdNode>) -> Result<(), String> {
    let mut stack: Vec<IdNode> = seq
        .iter()
        .filter_map(|item| match item {
            Item::Node(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    let mut empty = Vec::new();
    while let Some(node) = stack.pop() {
        match node.node_type() {
            NodeType::Document => stack.extend(node.child_iter()),
            NodeType::Element => {
                if node.first_child().is_none() {
                    if needs_end_tag(&node) {
                        empty.push(node);
                    }
                } else {
                    stack.extend(node.child_iter());
                }
            }
            _ => {}
        }
    }
    // Mutate after the walk, never during it.
    for mut element in empty {
        let text = element
            .new_text(Rc::new(Value::from("")))
            .map_err(|e| format!("html serialization: {}", e.message))?;
        element
            .push(text)
            .map_err(|e| format!("html serialization: {}", e.message))?;
    }
    Ok(())
}

/// An element HTML would misread as `<x/>`: in no namespace, and not void.
fn needs_end_tag(element: &IdNode) -> bool {
    element.name().is_some_and(|qn| {
        qn.namespace_uri().is_none() && {
            let local = qn.local_name().to_string().to_ascii_lowercase();
            !VOID.contains(&local.as_str())
        }
    })
}
