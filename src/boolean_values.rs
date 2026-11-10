//! The result kind of an XPath expression, for `numeric_values` (#2668).
//! Inspect the parsed XPath result type, not comparison tokens in predicates or strings.
//! Variable results that may change type (parameters, ambiguous bindings) are Unknown.
//! (Through 0.8.0 this also adapted boolean `value-of` results, which xrust 2.3.0 wrote as
//! empty strings, #2520; the carried xrust writes `true` and `false`, ledger #3343.)

use crate::{node_id::IdNode, XSL_NS};
use qualname::QName;
use xrust::{
    item::{Item, Node},
    parser::xpath::parse,
    transform::Transform,
    value::ValueData,
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Boolean,
    Other,
    Unknown,
}

fn is_xsl(n: &IdNode, local: &str) -> bool {
    n.name().is_some_and(|q| {
        q.local_name().to_string() == local
            && q.namespace_uri().is_some_and(|u| u.to_string() == XSL_NS)
    })
}

fn attr(n: &IdNode, name: &str) -> String {
    n.get_attribute(&QName::try_from(name).unwrap()).to_string()
}

// Resolve lexical bindings: preceding siblings at each enclosing scope, then globals.
fn binding(node: &IdNode, name: &str) -> Option<IdNode> {
    let mut current = node.clone();
    while let Some(parent) = current.parent() {
        let global = is_xsl(&parent, "stylesheet") || is_xsl(&parent, "transform");
        let mut found = None;
        for child in parent.child_iter() {
            if !global && child == current {
                break;
            }
            if (is_xsl(&child, "variable") || is_xsl(&child, "param"))
                && attr(&child, "name") == name
            {
                found = Some(child);
            }
        }
        if found.is_some() {
            return found;
        }
        current = parent;
    }
    None
}

// Shared with numeric/string conversion: resolve variable and parameter types in
// their lexical scope using the same bounded analysis as direct value-of.
pub(crate) fn expression_kind(source: &str, node: &IdNode) -> Kind {
    parse(source, Some(node.clone()), None)
        .map(|t| kind(&t, node, &mut Vec::new()))
        .unwrap_or(Kind::Unknown)
}

fn kind(t: &Transform<IdNode>, node: &IdNode, seen: &mut Vec<IdNode>) -> Kind {
    kind_in(t, node, seen, Kind::Other)
}

fn kind_in(t: &Transform<IdNode>, node: &IdNode, seen: &mut Vec<IdNode>, input: Kind) -> Kind {
    use Transform::*;
    match t {
        // Postfix expressions, including true() and $v, are Compose([value, Compose([])]).
        // An empty composition is an identity; it does not turn an atomic result into nodes.
        Compose(steps) => steps
            .iter()
            .fold(input, |k, step| kind_in(step, node, seen, k)),
        ContextItem => input,
        Filter(_) => {
            if input == Kind::Other {
                input
            } else {
                Kind::Unknown
            }
        }
        Or(_)
        | And(_)
        | GeneralComparison(..)
        | ValueComparison(..)
        | StartsWith(..)
        | EndsWith(..)
        | Contains(..)
        | Boolean(_)
        | Not(_)
        | True
        | False => Kind::Boolean,
        Literal(Item::Value(v)) if matches!(v.value_ref(), ValueData::Boolean(_)) => Kind::Boolean,
        VariableReference(name, _) => {
            let Some(declaration) = binding(node, name) else {
                return Kind::Unknown;
            };
            // Alias chains need a bound even when every individual XPath is shallow.
            if seen.len() >= crate::limits::MAX_EXPR_DEPTH || seen.contains(&declaration) {
                return Kind::Unknown;
            }
            seen.push(declaration.clone());
            let result = declaration_kind(&declaration, seen);
            let result = if is_xsl(&declaration, "param") {
                let global = declaration
                    .parent()
                    .is_some_and(|p| is_xsl(&p, "stylesheet") || is_xsl(&p, "transform"));
                if global {
                    // Request arguments supply strings, regardless of the default.
                    if result == Kind::Other {
                        result
                    } else {
                        Kind::Unknown
                    }
                } else {
                    local_parameter_kind(&declaration, seen, result)
                }
            } else {
                result
            };
            seen.pop();
            result
        }
        SequenceItems(items) if items.len() == 1 => kind(&items[0], node, seen),
        SequenceItems(items) => {
            if items.iter().any(|t| kind(t, node, seen) != Kind::Other) {
                Kind::Unknown
            } else {
                Kind::Other
            }
        }
        Switch(clauses, otherwise) => {
            let result = kind(otherwise, node, seen);
            if clauses
                .iter()
                .all(|(_, body)| kind(body, node, seen) == result)
            {
                result
            } else {
                Kind::Unknown
            }
        }
        Invoke(..) | Call(..) => Kind::Unknown,
        _ => Kind::Other,
    }
}

// A missing select denotes a result tree fragment or an empty string.
fn declaration_kind(node: &IdNode, seen: &mut Vec<IdNode>) -> Kind {
    match node.get_attribute_node(&QName::try_from("select").unwrap()) {
        None => Kind::Other,
        Some(a) => parse(&a.value().to_string(), Some(node.clone()), None)
            .map(|t| kind(&t, node, seen))
            .unwrap_or(Kind::Unknown),
    }
}

fn local_parameter_kind(param: &IdNode, seen: &mut Vec<IdNode>, default: Kind) -> Kind {
    let Some(template) = param.parent().filter(|p| is_xsl(p, "template")) else {
        return Kind::Unknown;
    };
    let template_name = attr(&template, "name");
    let param_name = attr(param, "name");
    let mut root = template;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    // Named calls reach only their named template. Any apply-templates with a
    // matching with-param is considered a possible caller, regardless of mode or
    // select: proving which source nodes reach a match rule is not needed here.
    let mut stack = vec![root];
    let mut result = default;
    while let Some(node) = stack.pop() {
        stack.extend(node.child_iter());
        if !(is_xsl(&node, "apply-templates")
            || (is_xsl(&node, "call-template")
                && !template_name.is_empty()
                && attr(&node, "name") == template_name))
        {
            continue;
        }
        for supplied in node
            .child_iter()
            .filter(|n| is_xsl(n, "with-param") && attr(n, "name") == param_name)
        {
            // Resolve aliases at the call site, not in the callee's scope. Keep
            // param in `seen` until all incoming values are inspected, so cycles
            // and long forwarding chains obey the same bound as variable aliases.
            if declaration_kind(&supplied, seen) != result {
                result = Kind::Unknown;
            }
            if result == Kind::Unknown {
                return result;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_stylesheet_value_of_kind_counts() {
        // These are the unmodified fixture bodies, including the CMS template
        // library before a consumer adds its caller. Pin every value-of site,
        // even those not reached by the synthetic integration-test documents.
        for (name, stylesheet, expected) in [
            (
                "cms-links",
                include_str!("../tests/fixtures/cms-links.xsl"),
                [0, 5, 0],
            ),
            (
                "gonk",
                include_str!("../tests/fixtures/gonk.xsl"),
                [0, 527, 0],
            ),
            (
                "rss-items",
                include_str!("../tests/fixtures/rss-items.xsl"),
                [0, 23, 0],
            ),
        ] {
            let doc = crate::parse_xml(stylesheet).unwrap();
            let mut stack = vec![doc.clone()];
            let mut counts = [0; 3];
            while let Some(node) = stack.pop() {
                stack.extend(node.child_iter());
                if is_xsl(&node, "value-of") {
                    let expr = parse(&attr(&node, "select"), Some(node.clone()), None).unwrap();
                    let index = match kind(&expr, &node, &mut Vec::new()) {
                        Kind::Boolean => 0,
                        Kind::Other => 1,
                        Kind::Unknown => 2,
                    };
                    counts[index] += 1;
                }
            }
            assert_eq!(counts, expected, "{name}: Boolean / Other / Unknown");
        }
    }
}
