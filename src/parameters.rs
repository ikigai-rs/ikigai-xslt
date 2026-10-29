//! Adapt top-level parameters to xrust's stylesheet variables. xrust 2.3 ignores
//! top-level xsl:param. Each becomes a variable with a lazy choice between a
//! per-run string binding and the original default; dependent variables therefore
//! see the chosen value, without recompiling or changing the stylesheet memo key.

use crate::{node_id::IdNode, XSL_NS};
use ikigai_core::{Error, Result};
use qualname::{NamespaceUri, NcName, QName};
use std::collections::BTreeMap;
use std::rc::Rc;
use xrust::item::{Item, Node};
use xrust::transform::context::Context;
use xrust::value::Value;
use xrust::xdmerror::Error as XsltError;

pub(crate) struct Parameter {
    name: String,
    supplied: String,
    value: String,
}

pub(crate) fn prepare(doc: &IdNode) -> Result<Vec<Parameter>> {
    let Some(root) = doc.child_iter().find(Node::is_element) else {
        return Ok(vec![]);
    };
    let param_name = format!("{{{XSL_NS}}}param");
    let declarations: Vec<_> = root
        .child_iter()
        .filter(|n| n.name().is_some_and(|name| name.to_string() == param_name))
        .collect();
    if declarations.is_empty() {
        return Ok(vec![]);
    }
    // The prefix is absent even from source expressions, so private bindings cannot
    // shadow a stylesheet variable, including one referenced only in an XPath.
    // Serialization normalizes character/entity references in names and XPath,
    // which could otherwise hide a collision from a scan of the original bytes.
    let stylesheet = doc.to_xml();
    let mut private = "ikigai_xslt_parameter_".to_string();
    while stylesheet.contains(&private) {
        private.push('_');
    }
    let mut parameters = Vec::<Parameter>::new();
    for mut param in declarations {
        let name = param
            .get_attribute(&QName::try_from("name").unwrap())
            .to_string();
        // xrust resolves variable references by local name, so namespace-qualified
        // parameter names cannot safely be represented by its variable machinery.
        if NcName::try_from(name.as_str()).is_err() {
            return Err(Error::InvalidArgument {
                name: "stylesheet".into(),
                detail: "top-level xsl:param requires an unprefixed XML name".into(),
            });
        }
        if parameters.iter().any(|p| p.name == name) {
            return Err(Error::InvalidArgument {
                name: "stylesheet".into(),
                detail: format!("duplicate top-level xsl:param `{name}`"),
            });
        }
        let parameter = Parameter {
            name,
            supplied: format!("{private}{}s", parameters.len()),
            value: format!("{private}{}v", parameters.len()),
        };
        adapt(&mut param, &parameter).map_err(|e| Error::Endpoint(e.message))?;
        parameters.push(parameter);
    }
    Ok(parameters)
}

fn element(doc: &IdNode, local: &str) -> std::result::Result<IdNode, XsltError> {
    doc.new_element(QName::new_from_parts(
        NcName::try_from(local).unwrap(),
        Some(NamespaceUri::try_from(XSL_NS).unwrap()),
    ))
}

fn attribute(node: &IdNode, name: &str, value: &str) -> std::result::Result<(), XsltError> {
    node.add_attribute(
        node.new_attribute(QName::try_from(name).unwrap(), Rc::new(Value::from(value)))?,
    )
}

fn adapt(param: &mut IdNode, binding: &Parameter) -> std::result::Result<(), XsltError> {
    let mut variable = element(param, "variable")?;
    for ns in param.namespace_iter() {
        variable.add_namespace(ns.deep_copy()?)?;
    }
    for attr in param
        .attribute_iter()
        .filter(|a| !a.name().is_some_and(|n| n.to_string() == "select"))
    {
        variable.add_attribute(attr.deep_copy()?)?;
    }
    let mut choose = element(param, "choose")?;
    let mut when = element(param, "when")?;
    attribute(&when, "test", &format!("${}", binding.supplied))?;
    // xsl:sequence compiles to the select expression itself in xrust. Unlike
    // copy-of, it preserves atomic types and the identity of default source nodes.
    let sequence = element(param, "sequence")?;
    attribute(&sequence, "select", &format!("${}", binding.value))?;
    when.push(sequence)?;
    choose.push(when)?;
    let mut otherwise = element(param, "otherwise")?;
    if let Some(select) = param.get_attribute_node(&QName::try_from("select").unwrap()) {
        let sequence = element(param, "sequence")?;
        attribute(&sequence, "select", &select.value().to_string())?;
        otherwise.push(sequence)?;
    } else {
        for child in param.child_iter().collect::<Vec<_>>() {
            otherwise.push(child)?;
        }
    }
    choose.push(otherwise)?;
    variable.push(choose)?;
    param.insert_before(variable)?;
    param.pop()
}

pub(crate) fn validate(parameters: &[Parameter], values: &BTreeMap<String, String>) -> Result<()> {
    for name in values.keys() {
        if !parameters.iter().any(|p| &p.name == name) {
            return Err(Error::InvalidArgument {
                name: name.clone(),
                detail: "no top-level xsl:param of this name is declared by the stylesheet".into(),
            });
        }
    }
    Ok(())
}

pub(crate) fn bind(
    parameters: &[Parameter],
    values: &BTreeMap<String, String>,
    ctxt: &Context<IdNode>,
) {
    for parameter in parameters {
        let value = values.get(&parameter.name);
        ctxt.var_push(
            parameter.supplied.clone(),
            vec![Item::Value(Rc::new(Value::from(value.is_some())))],
        );
        ctxt.var_push(
            parameter.value.clone(),
            vec![Item::Value(Rc::new(Value::from(
                value.map_or("", String::as_str),
            )))],
        );
    }
}
