//! Static stylesheet dependencies. Resolution is asynchronous; assembly is pure and
//! runs on the same bounded stack and deadline as compilation.

use crate::{limits, node_id::IdNode, parse_xml, resolve_ref, utf8, XSL_NS};
use ikigai_core::{Error, Invocation, Result};
use qualname::QName;
use std::collections::BTreeMap;
use xrust::item::Node;

/// Maximum number of import/include edges along a stylesheet's dependency path.
pub const MAX_MODULE_DEPTH: usize = 16;
/// Maximum number of module occurrences in one assembled stylesheet, including repeats.
pub const MAX_MODULE_OCCURRENCES: usize = 128;

#[derive(Clone, Default)]
pub(crate) struct Modules {
    pub base: Option<String>,
    pub texts: BTreeMap<String, String>,
}

pub(crate) fn invalid(detail: impl Into<String>) -> Error {
    Error::InvalidArgument {
        name: "stylesheet".into(),
        detail: detail.into(),
    }
}

fn engine(error: xrust::xdmerror::Error) -> Error {
    Error::Endpoint(format!("stylesheet module error: {}", error.message))
}

fn root(doc: &IdNode) -> Result<IdNode> {
    let mut elements = doc.child_iter().filter(Node::is_element);
    let root = elements
        .next()
        .ok_or_else(|| invalid("stylesheet module has no document element"))?;
    if elements.next().is_some() {
        return Err(invalid(
            "stylesheet module requires exactly one document element",
        ));
    }
    if !["stylesheet", "transform"]
        .iter()
        .any(|local| named(&root, local))
    {
        return Err(invalid(
            "stylesheet module requires an xsl:stylesheet or xsl:transform document element",
        ));
    }
    Ok(root)
}

fn named(node: &IdNode, local: &str) -> bool {
    node.name()
        .is_some_and(|name| name.to_string() == format!("{{{XSL_NS}}}{local}"))
}

fn href(node: &IdNode) -> Result<String> {
    let href = node
        .get_attribute(&QName::try_from("href").unwrap())
        .to_string();
    if href.is_empty() {
        return Err(invalid(format!(
            "{} requires a nonempty href",
            node.name().unwrap()
        )));
    }
    Ok(href)
}

fn references(doc: &IdNode) -> Result<Vec<String>> {
    root(doc)?
        .child_iter()
        .filter(|n| named(n, "include") || named(n, "import"))
        .map(|n| href(&n))
        .collect()
}

fn parse(text: &str) -> Result<IdNode> {
    limits::check_stylesheet(text, "stylesheet")?;
    parse_xml(text)
        .map_err(|error| Error::Endpoint(format!("stylesheet parse error: {}", error.message)))
}

// Store only static hrefs, never fetched resources or xrust trees. Without this
// small content-keyed memo every endpoint call would reparse a reused stylesheet
// before reaching the compile memo. A module edit gets its own key.
static HREFS: std::sync::Mutex<Vec<(String, Vec<String>)>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn clear_cache() {
    HREFS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn scan(text: String) -> Result<Vec<String>> {
    {
        let mut cache = HREFS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(index) = cache.iter().position(|(key, _)| key == &text) {
            let entry = cache.remove(index);
            let hrefs = entry.1.clone();
            cache.insert(0, entry);
            return Ok(hrefs);
        }
    }
    limits::check_stylesheet(&text, "stylesheet")?;
    let xml = text.clone();
    let hrefs = limits::on_xslt_stack_within(limits::DEFAULT_TIME_BUDGET, move || {
        references(&parse(&xml)?)
    })??;
    let mut cache = HREFS.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(key, _)| key != &text);
    cache.insert(0, (text, hrefs.clone()));
    cache.truncate(crate::STYLESHEET_CACHE_ENTRIES);
    Ok(hrefs)
}

pub(crate) fn resolve(base: Option<&str>, href: &str) -> Result<String> {
    match base {
        Some(base) => oxiri::Iri::parse(base).map_err(|e| invalid(format!("stylesheet IRI `{base}` for href `{href}` is invalid: {e}")))?
            .resolve(href).map(|iri| iri.into_inner())
            .map_err(|e| invalid(format!("stylesheet href `{href}` is invalid: {e}"))),
        None => oxiri::Iri::parse(href).map(|iri| iri.into_inner().to_string())
            .map_err(|_| invalid(format!("relative or invalid stylesheet href `{href}` has no base IRI; use an absolute href or a stylesheet resource reference"))),
    }
}

fn check_path(chain: &[String], iri: &str) -> Result<()> {
    let mut path = chain.to_vec();
    path.push(iri.to_string());
    let path = path.join(" -> ");
    if chain.iter().any(|ancestor| ancestor == iri) {
        return Err(invalid(format!(
            "stylesheet module cycle: {path}; remove the cyclic import or include"
        )));
    }
    if chain.len() > MAX_MODULE_DEPTH {
        return Err(invalid(format!("stylesheet module path exceeds depth {MAX_MODULE_DEPTH}: {path}; shorten the import/include chain")));
    }
    Ok(())
}

pub(crate) async fn fetch(
    inv: &Invocation<'_>,
    base: Option<&str>,
    stylesheet: &str,
) -> Result<Modules> {
    let mut modules = Modules {
        base: base.map(str::to_string),
        texts: BTreeMap::new(),
    };
    let chain = base
        .map(|base| vec![base.to_string()])
        .unwrap_or_else(|| vec!["inline stylesheet".into()]);
    let mut pending = vec![(base.map(str::to_string), stylesheet.to_string(), chain)];
    let mut occurrences = 0;
    while let Some((base, text, chain)) = pending.pop() {
        for href in scan(text)? {
            let iri = resolve(base.as_deref(), &href)?;
            check_path(&chain, &iri)?;
            occurrences += 1;
            if occurrences > MAX_MODULE_OCCURRENCES {
                return Err(invalid(format!("stylesheet has more than {MAX_MODULE_OCCURRENCES} module occurrences; reduce repeated imports/includes")));
            }
            let text = if let Some(text) = modules.texts.get(&iri) {
                text.clone()
            } else {
                let repr = resolve_ref(inv, &iri).await.map_err(|error| match error {
                    Error::Unresolved(_) | Error::NotFound(_) => Error::NotFound(format!(
                        "stylesheet module href `{href}` (`{iri}`) from {}",
                        chain.join(" -> ")
                    )),
                    other => other,
                })?;
                let text = utf8(repr, &format!("stylesheet module `{href}` (`{iri}`)"))?;
                limits::check_stylesheet(&text, "stylesheet")?;
                modules.texts.insert(iri.clone(), text.clone());
                text
            };
            let mut path = chain.clone();
            path.push(iri.clone());
            pending.push((Some(iri), text, path));
        }
    }
    Ok(modules)
}

impl Modules {
    /// Length prefixes make the content key unambiguous, including base and every
    /// dependency's bytes. A cut followed by changed module bytes cannot reuse a compile.
    pub fn key(&self, stylesheet: &str) -> String {
        if self.texts.is_empty() {
            return format!("stylesheet:{}:{stylesheet}", stylesheet.len());
        }
        let mut key = String::from("modules:");
        for part in std::iter::once(stylesheet)
            .chain(std::iter::once(self.base.as_deref().unwrap_or("")))
            .chain(
                self.texts
                    .iter()
                    .flat_map(|(iri, text)| [iri.as_str(), text.as_str()]),
            )
        {
            key.push_str(&format!("{}:{part}", part.len()));
        }
        key
    }

    pub fn assemble(&self, stylesheet: &str) -> Result<IdNode> {
        // xrust expands only the outermost modules, after reading xsl:output and
        // without applying this crate's guards to imported code. Assemble first so
        // every declaration is checked and adapted together before compilation.
        let doc = parse(stylesheet)?;
        let mut root = root(&doc)?;
        if references(&doc)?.is_empty() {
            return Ok(doc);
        }
        let chain = vec![self
            .base
            .clone()
            .unwrap_or_else(|| "inline stylesheet".into())];
        let mut assembly = Assembly::default();
        let mut declarations =
            self.collect(stylesheet, self.base.as_deref(), &chain, 0, &mut assembly)?;
        assembly.finish(0);
        if assembly.precedence.len() > 1 {
            prepare_precedence(&mut declarations, &assembly.precedence)?;
        }
        let children: Vec<_> = root.child_iter().collect();
        for mut child in children {
            child.pop().map_err(engine)?;
        }
        for declaration in declarations {
            root.push(declaration.node).map_err(engine)?;
        }
        Ok(doc)
    }

    fn collect(
        &self,
        text: &str,
        base: Option<&str>,
        chain: &[String],
        group: usize,
        assembly: &mut Assembly,
    ) -> Result<Vec<Declaration>> {
        let doc = parse(text)?;
        let root = root(&doc)?;
        assembly.documents.push(doc);
        let mut declarations = Vec::new();
        for child in root.child_iter() {
            if !(named(&child, "include") || named(&child, "import")) {
                let copy = child.deep_copy().map_err(engine)?;
                // Preserve XPath namespace environments when detaching from the module.
                if copy.is_element() {
                    for ns in child.namespace_iter() {
                        copy.add_namespace(ns.deep_copy().map_err(engine)?)
                            .map_err(engine)?;
                    }
                }
                declarations.push(Declaration { node: copy, group });
                continue;
            }
            let href = href(&child)?;
            let iri = resolve(base, &href)?;
            check_path(chain, &iri)?;
            assembly.occurrences += 1;
            if assembly.occurrences > MAX_MODULE_OCCURRENCES {
                return Err(invalid(format!("stylesheet has more than {MAX_MODULE_OCCURRENCES} module occurrences; reduce repeated imports/includes")));
            }
            let text = self.texts.get(&iri).ok_or_else(|| {
                Error::NotFound(format!(
                    "stylesheet module href `{href}` (`{iri}`) was not supplied"
                ))
            })?;
            let mut path = chain.to_vec();
            path.push(iri.clone());
            let imported = named(&child, "import");
            let child_group = if imported {
                assembly.precedence.push(0);
                assembly.precedence.len() - 1
            } else {
                group
            };
            declarations.extend(self.collect(text, Some(&iri), &path, child_group, assembly)?);
            if imported {
                assembly.finish(child_group);
            }
        }
        Ok(declarations)
    }
}

struct Declaration {
    node: IdNode,
    group: usize,
}

struct Assembly {
    // xrust parent/owner links are weak until declarations are attached.
    documents: Vec<IdNode>,
    occurrences: usize,
    precedence: Vec<usize>,
    next_precedence: usize,
}

impl Default for Assembly {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            occurrences: 0,
            precedence: vec![0],
            next_precedence: 0,
        }
    }
}

impl Assembly {
    fn finish(&mut self, group: usize) {
        self.precedence[group] = self.next_precedence;
        self.next_precedence += 1;
    }
}

fn attribute(node: &IdNode, name: &str) -> String {
    node.get_attribute(&QName::try_from(name).unwrap())
        .to_string()
}

fn set_attribute(node: &IdNode, name: &str, value: &str) -> Result<()> {
    if let Some(mut old) = node.get_attribute_node(&QName::try_from(name).unwrap()) {
        old.pop().map_err(engine)?;
    }
    node.add_attribute(
        node.new_attribute(
            QName::try_from(name).unwrap(),
            std::rc::Rc::new(xrust::value::Value::from(value)),
        )
        .map_err(engine)?,
    )
    .map_err(engine)
}

/// xrust orders matched templates by priority alone, ignoring import precedence.
/// Encode the pair as a finite ordinal, preserving default priorities within a level.
/// Named templates and global bindings instead discard shadowed declarations.
fn prepare_precedence(declarations: &mut Vec<Declaration>, precedence: &[usize]) -> Result<()> {
    let mut bindings = BTreeMap::<String, usize>::new();
    let mut names = BTreeMap::<String, usize>::new();
    let mut outputs = Vec::new();
    let mut priorities = Vec::new();
    for (index, declaration) in declarations.iter().enumerate() {
        let node = &declaration.node;
        let rank = precedence[declaration.group];
        if named(node, "param") || named(node, "variable") {
            let name = attribute(node, "name");
            let entry = bindings.entry(name).or_insert(rank);
            *entry = (*entry).max(rank);
        }
        if named(node, "template") {
            let name = attribute(node, "name");
            if !name.is_empty() {
                let entry = names.entry(name).or_insert(rank);
                *entry = (*entry).max(rank);
            }
            if !attribute(node, "match").is_empty() {
                let priority = default_priority(node)?;
                priorities.push((index, rank, priority));
            }
        }
        if named(node, "output") {
            outputs.push((index, rank));
        }
        if contains_apply_imports(node) {
            return Err(invalid("xsl:apply-imports is not supported by the module compiler; use distinct modes or named templates"));
        }
    }
    priorities.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)));
    let mut ordinal = 0;
    let mut previous = None;
    for (index, rank, priority) in priorities {
        if previous.is_some_and(|pair| pair != (rank, priority)) {
            ordinal += 1;
        }
        set_attribute(&declarations[index].node, "priority", &ordinal.to_string())?;
        previous = Some((rank, priority));
    }
    // Merge output attributes from low to high precedence, in document order.
    // xrust reads the first xsl:output only, and the endpoint must read the same one.
    outputs.sort_by_key(|&(index, rank)| (rank, index));
    let mut output_attributes = BTreeMap::new();
    for &(index, _) in &outputs {
        for attr in declarations[index].node.attribute_iter() {
            if let Some(name) = attr.name() {
                output_attributes.insert(name, attr.value());
            }
        }
    }
    let first_output = outputs.iter().map(|&(index, _)| index).min();
    let mut index = 0;
    declarations.retain_mut(|declaration| {
        let node = &declaration.node;
        let rank = precedence[declaration.group];
        let keep = if named(node, "param") || named(node, "variable") {
            bindings.get(&attribute(node, "name")) == Some(&rank)
        } else if named(node, "output") {
            Some(index) == first_output
        } else {
            true
        };
        index += 1;
        keep
    });
    for declaration in declarations {
        let node = &declaration.node;
        if named(node, "output") {
            for (name, value) in &output_attributes {
                if let Some(mut old) = node.get_attribute_node(name) {
                    old.pop().map_err(engine)?;
                }
                node.add_attribute(
                    node.new_attribute(name.clone(), value.clone())
                        .map_err(engine)?,
                )
                .map_err(engine)?;
            }
        }
        if named(node, "template") {
            let name = attribute(node, "name");
            if !name.is_empty() && names.get(&name) != Some(&precedence[declaration.group]) {
                // A template can have both a name and a match. Shadow only its name.
                if let Some(mut attr) = node.get_attribute_node(&QName::try_from("name").unwrap()) {
                    attr.pop().map_err(engine)?;
                }
            }
        }
    }
    Ok(())
}

fn contains_apply_imports(node: &IdNode) -> bool {
    named(node, "apply-imports")
        || node
            .child_iter()
            .any(|child| contains_apply_imports(&child))
}

// The same default-priority calculation as xrust 2.3.0's compiler, on its parsed
// pattern rather than a second lexical interpretation of XPath.
fn default_priority(node: &IdNode) -> Result<f64> {
    use xrust::pattern::{Branch, Pattern};
    use xrust::transform::{Axis, NameTest, NodeTest, Transform};
    let explicit = attribute(node, "priority");
    if !explicit.is_empty() {
        return explicit
            .parse::<f64>()
            .ok()
            .filter(|p| p.is_finite())
            .ok_or_else(|| {
                invalid(format!(
                    "xsl:template priority `{explicit}` must be a finite number"
                ))
            });
    }
    let pattern: Pattern<IdNode> =
        Pattern::try_from((attribute(node, "match"), node.clone())).map_err(engine)?;
    Ok(match pattern {
        Pattern::Predicate(Transform::Empty) => -1.0,
        Pattern::Predicate(_) => 1.0,
        Pattern::Selection(Branch::Error(error)) => return Err(engine(error)),
        Pattern::Selection(branch) => {
            let (axis, parent, test) = branch.terminal_node_test();
            match (axis, parent) {
                (Axis::SelfAttribute, _) => -0.5,
                (Axis::SelfAxis, Axis::Parent | Axis::Ancestor | Axis::AncestorOrSelf) => {
                    match test {
                        NodeTest::Name(NameTest::Wildcard(_, _)) | NodeTest::Kind(_) => -0.5,
                        _ => 0.0,
                    }
                }
                _ => 0.5,
            }
        }
        _ => -1.0,
    })
}
