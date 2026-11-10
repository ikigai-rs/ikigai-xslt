/*! # Whitespace Management.

A stylesheet may specify that whitespace in a document is to be stripped.
This module provides functions to strip whitespace from a document.
Note that this is a destructive process - text nodes will be removed from the document tree.
*/

use crate::item::{Node, NodeType};
use crate::transform::{KindTest, NameTest, NodeTest, WildcardOrName, WildcardOrNamespaceUri};
use crate::xdmerror::Error;

/// Strip whitespace nodes from a XDM tree.
/// See [XSLT 4.3](https://www.w3.org/TR/2017/REC-xslt-30-20170608/#stylesheet-stripping).
/// The [Node] argument must be the document node of the tree.
pub fn strip_whitespace<N: Node>(
    t: N,
    cpi: bool, // strip comments and PIs?
    strip: &Vec<NodeTest>,
    preserve: &Vec<NodeTest>,
) -> Result<(), Error> {
    t.child_iter().try_for_each(|n| {
        strip_whitespace_node(n, cpi, strip, preserve, true)?;
        Ok(())
    })?;
    Ok(())
}

// TODO: the rules for stripping/preserving are a lot more complex
fn strip_whitespace_node<N: Node>(
    mut n: N,
    cpi: bool, // strip comments and PIs?
    strip: &Vec<NodeTest>,
    preserve: &Vec<NodeTest>,
    keep: bool,
) -> Result<(), Error> {
    match n.node_type() {
        NodeType::Comment | NodeType::ProcessingInstruction => {
            if cpi {
                n.pop()?;
                // TODO: Merge text nodes that are now adjacent
            }
        }
        NodeType::Element => {
            // Determine if this element toggles the strip/preserve setting
            // Match a strip NodeTest or a preserve NodeTest
            // The 'strength' of the match determines which setting wins
            let mut ss = -1.0;
            let mut ps = -1.0;
            strip.iter().for_each(|t| match t {
                NodeTest::Kind(KindTest::Any) | NodeTest::Kind(KindTest::Element) => ss = -0.5,
                NodeTest::Name(nt) => match nt {
                    NameTest::Wildcard(
                        WildcardOrNamespaceUri::Wildcard,
                        WildcardOrName::Name(_),
                    ) => ss = -0.25,
                    NameTest::Wildcard(
                        WildcardOrNamespaceUri::NamespaceUri(_),
                        WildcardOrName::Wildcard,
                    ) => ss = -0.25,
                    NameTest::Wildcard(_, _) => ss = -0.5,
                    NameTest::Name(qn) => {
                        if *qn == n.name().unwrap() {
                            ss = 0.5
                        }
                    }
                },
                _ => {}
            });
            preserve.iter().for_each(|t| match t {
                NodeTest::Kind(KindTest::Any) | NodeTest::Kind(KindTest::Element) => ps = -0.5,
                NodeTest::Name(nt) => match nt {
                    NameTest::Wildcard(
                        WildcardOrNamespaceUri::Wildcard,
                        WildcardOrName::Name(_),
                    ) => ss = -0.25,
                    NameTest::Wildcard(
                        WildcardOrNamespaceUri::NamespaceUri(_),
                        WildcardOrName::Wildcard,
                    ) => ss = -0.25,
                    NameTest::Wildcard(_, _) => ss = -0.5,
                    NameTest::Name(qn) => {
                        if *qn == n.name().unwrap() {
                            ss = 0.5
                        }
                    }
                },
                _ => {}
            });
            n.child_iter().try_for_each(|m| {
                strip_whitespace_node(
                    m,
                    cpi,
                    strip,
                    preserve,
                    if ss > -1.0 {
                        ps >= ss
                    } else if ps > -1.0 {
                        true
                    } else {
                        keep
                    },
                )
            })?
        }
        NodeType::Text => {
            if n.to_string().trim().is_empty() && !keep {
                n.pop()?;
            }
        }
        _ => {}
    }
    Ok(())
}
