//! Refuse output declarations the serializer cannot honor, rather than ignore them.
use crate::{node_id::IdNode, XSL_NS};
use ikigai_core::{Error, Result};
use xrust::item::Node;

pub(crate) fn check(doc: &IdNode) -> Result<()> {
    let mut stack = vec![doc.clone()];
    while let Some(node) = stack.pop() {
        stack.extend(node.child_iter());
        let Some(name) = node.name() else { continue };
        if !name
            .namespace_uri()
            .is_some_and(|ns| ns.to_string() == XSL_NS)
        {
            continue;
        }
        let local = name.local_name().to_string();
        if local == "namespace-alias" {
            return Err(refused("xsl:namespace-alias is not supported; write literal result names in the intended namespace"));
        }
        if local != "output" {
            continue;
        }
        for attr in node.attribute_iter() {
            let name = attr.name().unwrap();
            if name.namespace_uri().is_some() {
                continue;
            }
            let local = name.local_name().to_string();
            let value = attr.value().to_string();
            match local.as_str() {
                "encoding" if !value.eq_ignore_ascii_case("utf-8") => {
                    return Err(refused(&format!(
                        "xsl:output encoding=\"{value}\" is not supported; use UTF-8"
                    )));
                }
                "omit-xml-declaration" if value != "yes" => {
                    return Err(refused(&format!("xsl:output omit-xml-declaration=\"{value}\" is not supported; use yes and supply a declaration in the host if needed")));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn refused(detail: &str) -> Error {
    Error::InvalidArgument {
        name: "stylesheet".into(),
        detail: format!("refused: {detail}"),
    }
}
