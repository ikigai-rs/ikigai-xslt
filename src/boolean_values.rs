//! The result kind of an XPath expression, for `numeric_values` (#2668).
//! Inspect the parsed XPath result type, not comparison tokens in predicates or strings.
//! Variable results that may change type (parameters, ambiguous bindings) are Unknown.
//! Also makes a boolean `copy-of` construct text, and refuses an atomic `xsl:sequence`
//! that may be boolean, so neither leaks into a result tree fragment (#3412).
//! (Through 0.8.1 this also adapted boolean `value-of` results, which xrust 2.3.0 wrote as
//! empty strings, #2520; the carried xrust writes `true` and `false`, ledger #3343.)
//! Declarations are compared by identity: xrust's `==` on nodes is structural, so two
//! templates' `<xsl:param name="iri"/>` would otherwise be one declaration (#3398).

use crate::{node_id::IdNode, XSL_NS};
use ikigai_core::{Error, Result};
use qualname::{NamespaceUri, NcName, QName};
use std::rc::Rc;
use xrust::{
    item::{Item, Node},
    parser::xpath::parse,
    transform::Transform,
    value::{Value, ValueData},
};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Boolean,
    Other,
    Unknown,
}

// A declaration under analysis. A local parameter reached again through its own
// template's calls answers the kind assumed for it; any other cycle is Unknown.
struct Visit {
    declaration: IdNode,
    assumed: Option<Kind>,
    read: bool,
}

// Bounds the whole analysis of one expression: alias chains by depth, and recursive
// parameters' passes by a total number of declaration visits.
struct Analysis {
    stack: Vec<Visit>,
    visits: usize,
}

const MAX_VISITS: usize = 16 * crate::limits::MAX_EXPR_DEPTH;

impl Analysis {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            visits: 0,
        }
    }
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
            if !global && child.is_same(&current) {
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
    source_kind(source, node, &mut Analysis::new()).unwrap_or(Kind::Unknown)
}

fn source_kind(source: &str, node: &IdNode, seen: &mut Analysis) -> Option<Kind> {
    parse(source, Some(node.clone()), None).ok().map(|t| {
        // xrust parses ex:string/ex:true/etc. as the corresponding core
        // Transform, losing the function namespace. Conservatively keep any
        // expression containing a prefixed call unknown. Tokens distinguish
        // calls from axis steps, QNames in paths, and quoted text.
        let extension = crate::numeric_values::tokens(source).is_some_and(|tokens| {
            tokens.windows(2).any(|pair| {
                pair[1] == "("
                    && pair[0].split_once(':').is_some_and(|(prefix, local)| {
                        NcName::try_from(prefix).is_ok() && NcName::try_from(local).is_ok()
                    })
            })
        });
        if extension {
            Kind::Unknown
        } else {
            kind(&t, node, seen)
        }
    })
}

// The result types of the XPath 1.0 core functions xrust 2.3.0 does not parse into
// a Transform of their own. Any other function, including extensions, is Unknown.
fn invoked_kind(name: &QName) -> Kind {
    if name.namespace_uri().is_some() {
        return Kind::Unknown;
    }
    match name.local_name().to_string().as_str() {
        "lang" => Kind::Boolean,
        "namespace-uri" => Kind::Other,
        _ => Kind::Unknown,
    }
}

fn kind(t: &Transform<IdNode>, node: &IdNode, seen: &mut Analysis) -> Kind {
    kind_in(t, node, seen, Kind::Other)
}

fn kind_in(t: &Transform<IdNode>, node: &IdNode, seen: &mut Analysis, input: Kind) -> Kind {
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
            reference_kind(&declaration, seen)
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
        Invoke(name, ..) => invoked_kind(name),
        // XPath 2.0 bindings are accepted by xrust's parser, but are not stylesheet
        // declarations. Their bodies can return booleans, including through a bound
        // variable, so do not let the nonboolean fallback admit them.
        Call(..) | Loop(..) | VariableDeclaration(..) => Kind::Unknown,
        _ => Kind::Other,
    }
}

fn reference_kind(declaration: &IdNode, seen: &mut Analysis) -> Kind {
    if let Some(visit) = seen
        .stack
        .iter_mut()
        .find(|v| v.declaration.is_same(declaration))
    {
        visit.read = true;
        return visit.assumed.unwrap_or(Kind::Unknown);
    }
    let global = declaration
        .parent()
        .is_some_and(|p| is_xsl(&p, "stylesheet") || is_xsl(&p, "transform"));
    let local_param = is_xsl(declaration, "param") && !global;
    // A recursive parameter's kind is a guess, string first, then boolean. The guess
    // holds when every call site, the recursive ones reading the guess, gives exactly
    // that kind: by induction on the recursion depth, no call passes anything else.
    let mut guesses = [Kind::Other, Kind::Boolean].into_iter();
    let mut assumed = guesses.next().unwrap();
    loop {
        // Alias chains need a bound even when every individual XPath is shallow.
        if seen.stack.len() >= crate::limits::MAX_EXPR_DEPTH || seen.visits >= MAX_VISITS {
            return Kind::Unknown;
        }
        seen.visits += 1;
        seen.stack.push(Visit {
            declaration: declaration.clone(),
            assumed: local_param.then_some(assumed),
            read: false,
        });
        let result = declaration_kind(declaration, seen);
        let result = if !is_xsl(declaration, "param") {
            result
        } else if global {
            // Request arguments supply strings, regardless of the default.
            if result == Kind::Other {
                result
            } else {
                Kind::Unknown
            }
        } else {
            local_parameter_kind(declaration, seen, result)
        };
        let visit = seen.stack.pop().unwrap();
        if !local_param || !visit.read || result == assumed {
            return result;
        }
        match guesses.next() {
            Some(guess) => assumed = guess,
            None => return Kind::Unknown,
        }
    }
}

// A missing select denotes a result tree fragment or an empty string.
fn declaration_kind(node: &IdNode, seen: &mut Analysis) -> Kind {
    match node.get_attribute_node(&QName::try_from("select").unwrap()) {
        None => Kind::Other,
        Some(a) => source_kind(&a.value().to_string(), node, seen).unwrap_or(Kind::Unknown),
    }
}

fn local_parameter_kind(param: &IdNode, seen: &mut Analysis, default: Kind) -> Kind {
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

// xrust stores an atomic boolean that copy-of or sequence returns inside a result tree
// fragment as that boolean, not as a text node, so `boolean($v)` of a variable holding
// `<xsl:copy-of select="false()"/>` would be false (#3412). A known boolean copy-of becomes
// xsl:choose with explicit text, evaluated once in the original context; xsl:sequence
// preserves atomic types by design, so a boolean or unknown one is refused, as is an unknown
// copy-of. (value-of needs none of this: the carried xrust writes `true` and `false`.)
pub(crate) fn prepare(doc: &IdNode) -> Result<()> {
    let mut stack = vec![doc.clone()];
    let mut booleans = Vec::new();
    while let Some(n) = stack.pop() {
        stack.extend(n.child_iter());
        let sequence = is_xsl(&n, "sequence");
        if !(is_xsl(&n, "copy-of") || sequence) {
            continue;
        }
        let select = attr(&n, "select");
        // Invalid XPath remains xrust's ordinary compile error.
        let Some(result) = source_kind(&select, &n, &mut Analysis::new()) else {
            continue;
        };
        match result {
            Kind::Boolean if !sequence => booleans.push((n, select)),
            Kind::Other => {},
            Kind::Boolean | Kind::Unknown => return Err(Error::InvalidArgument {
                name: "stylesheet".into(),
                detail: format!("refused: use xsl:choose with explicit true/false text, or supply a string. xsl:{} select=\"{select}\" may return a boolean, which xrust 2.3.0 would keep as an atomic value inside a result tree fragment instead of text; see README, The XSLT subset", n.name().unwrap().local_name().to_string()),
            }),
        }
    }
    for (mut node, select) in booleans {
        adapt(&mut node, &select).map_err(|e| Error::Endpoint(e.message))?;
    }
    Ok(())
}

fn element(doc: &IdNode, local: &str) -> std::result::Result<IdNode, xrust::xdmerror::Error> {
    doc.new_element(QName::new_from_parts(
        NcName::try_from(local).unwrap(),
        Some(NamespaceUri::try_from(XSL_NS).unwrap()),
    ))
}

fn adapt(node: &mut IdNode, select: &str) -> std::result::Result<(), xrust::xdmerror::Error> {
    let mut choose = element(node, "choose")?;
    for ns in node.namespace_iter() {
        choose.add_namespace(ns.deep_copy()?)?;
    }
    let mut when = element(node, "when")?;
    when.add_attribute(when.new_attribute(
        QName::try_from("test").unwrap(),
        Rc::new(Value::from(select)),
    )?)?;
    when.push(node.new_text(Rc::new(Value::from("true")))?)?;
    let mut otherwise = element(node, "otherwise")?;
    otherwise.push(node.new_text(Rc::new(Value::from("false")))?)?;
    choose.push(when)?;
    choose.push(otherwise)?;
    node.insert_before(choose)?;
    node.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_function_result_types() {
        let doc = crate::parse_xml(&format!(
            "<xsl:stylesheet xmlns:xsl='{XSL_NS}' xmlns:ex='urn:ex' version='1.0'><xsl:template match='/'/></xsl:stylesheet>"
        ))
        .unwrap();
        let node = doc.child_iter().next().unwrap();
        for (expected, functions) in [
            (
                Kind::Other,
                &[
                    "string(.)",
                    "concat('a', 'b')",
                    "substring('a', 1)",
                    "substring-before('a', 'b')",
                    "substring-after('a', 'b')",
                    "normalize-space('a')",
                    "translate('a', 'b', 'c')",
                    "local-name()",
                    "name()",
                    "namespace-uri()",
                    "count(*)",
                    "sum(*)",
                    "number('1')",
                    "string-length('a')",
                    "floor(1)",
                    "ceiling(1)",
                    "round(1)",
                    "position()",
                    "last()",
                ][..],
            ),
            (
                Kind::Boolean,
                &[
                    "boolean(*)",
                    "not(*)",
                    "true()",
                    "false()",
                    "lang('en')",
                    "contains('a', 'b')",
                    "starts-with('a', 'b')",
                ][..],
            ),
            (
                Kind::Unknown,
                &[
                    "ex:f()",
                    "ex:lang('en')",
                    "ex:string(true())",
                    "ex:true()",
                    "ex:number(true())",
                    "unparsed-entity-uri('a')",
                ][..],
            ),
        ] {
            for function in functions {
                assert!(expression_kind(function, &node) == expected, "{function}");
            }
        }
    }

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
                    let index = match kind(&expr, &node, &mut Analysis::new()) {
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
