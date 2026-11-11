//! Static stylesheet dependencies. Resolution is asynchronous; assembly is pure and
//! runs on the same bounded stack and deadline as compilation.

use crate::{limits, node_id::IdNode, parse_xml, resolve_ref, utf8, XSL_NS};
use ikigai_core::{Error, Invocation, Result};
use qualname::{NamespacePrefix, NcName, QName};
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
    let mut seen_declaration = false;
    for child in root.child_iter().filter(Node::is_element) {
        if named(&child, "import") {
            if seen_declaration {
                return Err(invalid("xsl:import must precede all other top-level elements in each stylesheet module; move imports to the beginning."));
            }
        } else {
            seen_declaration = true;
        }
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
    let mut bytes = 0;
    charge_bytes(&mut bytes, stylesheet.len())?;
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
                charge_bytes(&mut bytes, text.len())?;
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
                charge_bytes(&mut bytes, text.len())?;
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
        charge_bytes(&mut 0, stylesheet.len())?;
        let doc = parse(stylesheet)?;
        let mut root = root(&doc)?;
        let chain = vec![self
            .base
            .clone()
            .unwrap_or_else(|| "inline stylesheet".into())];
        let mut assembly = Assembly::default();
        let mut declarations = if references(&doc)?.is_empty() {
            assembly.bytes = stylesheet.len();
            root.child_iter()
                .map(|node| Declaration { node, group: 0 })
                .collect()
        } else {
            self.collect(stylesheet, self.base.as_deref(), &chain, 0, &mut assembly)?
        };
        assembly.finish(0);
        check_duplicate_names(&declarations)?;
        declarations = split_union_templates(declarations, &mut assembly.bytes)?;
        prepare_precedence(&mut declarations, &assembly.precedence)?;
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
        charge_bytes(&mut assembly.bytes, text.len())?;
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

// Count expanded source occurrences before allocating their trees. This deliberately
// includes module wrappers and comments, making the module budget conservative.
fn charge_bytes(total: &mut usize, amount: usize) -> Result<()> {
    let limit = limits::MAX_ASSEMBLED_STYLESHEET_BYTES;
    if amount > limit.saturating_sub(*total) {
        return Err(invalid(format!("The assembled stylesheet exceeds the limit of {limit} bytes; reduce stylesheet size or repeated imports/includes.")));
    }
    *total += amount;
    Ok(())
}

// Names in declarations are expanded QNames; the default namespace does not apply.
fn declaration_name(node: &IdNode) -> Result<String> {
    if !(named(node, "template") || named(node, "variable") || named(node, "param")) {
        return Ok(String::new());
    }
    let name = attribute(node, "name");
    if name.is_empty() {
        return Ok(name);
    }
    if let Some((prefix, local)) = name.split_once(':') {
        let prefix = NamespacePrefix::try_from(prefix)
            .map_err(|_| invalid(format!("declaration name `{name}` is not a QName")))?;
        let local = NcName::try_from(local)
            .map_err(|_| invalid(format!("declaration name `{name}` is not a QName")))?;
        let namespace = node.to_namespace_uri(&Some(prefix)).map_err(|_| {
            invalid(format!(
                "declaration name `{name}` uses an undeclared namespace prefix"
            ))
        })?;
        Ok(format!(
            "{{{}}}{}",
            namespace.to_string(),
            local.to_string()
        ))
    } else {
        NcName::try_from(name.as_str())
            .map_err(|_| invalid(format!("declaration name `{name}` is not a QName")))?;
        Ok(name)
    }
}

fn check_duplicate_names(declarations: &[Declaration]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for declaration in declarations {
        let node = &declaration.node;
        let name = declaration_name(node)?;
        if !name.is_empty()
            && !seen.insert((named(node, "template"), declaration.group, name.clone()))
        {
            let kind = if named(node, "template") {
                "named template"
            } else {
                "global variable or parameter"
            };
            return Err(invalid(format!("duplicate {kind} `{name}` at the same import precedence; keep only one declaration.")));
        }
    }
    Ok(())
}

// Split only union separators outside literals and nested expressions. A pipe in
// a processing-instruction target or predicate literal belongs to that expression.
fn alternatives(pattern: &str) -> Vec<&str> {
    let mut quote = None;
    let mut depth = 0usize;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, ch) in pattern.char_indices() {
        if let Some(delimiter) = quote {
            if ch == delimiter {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                parts.push(pattern[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(pattern[start..].trim());
    parts
}

// xrust's attribute deep_copy retains the old parent, so adding that copy
// removes the original attribute. Recreate attributes to preserve every union rule.
fn copy_template(node: &IdNode) -> Result<IdNode> {
    let mut copy = node.shallow_copy().map_err(engine)?;
    for attr in node.attribute_iter() {
        copy.add_attribute(
            copy.new_attribute(attr.name().unwrap(), attr.value())
                .map_err(engine)?,
        )
        .map_err(engine)?;
    }
    for child in node.child_iter() {
        copy.push(copy_template(&child)?).map_err(engine)?;
    }
    Ok(copy)
}

fn split_union_templates(
    declarations: Vec<Declaration>,
    bytes: &mut usize,
) -> Result<Vec<Declaration>> {
    let mut expanded = Vec::new();
    for declaration in declarations {
        let node = &declaration.node;
        let pattern = attribute(node, "match");
        let parts = alternatives(&pattern);
        if !named(node, "template") || !attribute(node, "priority").is_empty() || parts.len() == 1 {
            expanded.push(declaration);
            continue;
        }
        let copy_bytes = node.to_xml().len();
        for (index, part) in parts.iter().enumerate() {
            let copy = if index == 0 {
                node.clone()
            } else {
                charge_bytes(bytes, copy_bytes)?;
                let copy = copy_template(node)?;
                // The copy is detached, so it has no in-scope namespaces of its own. Give
                // it the original's (its own module's), so every alternative's prefixes
                // resolve as alternative 0's do, even where an importer binds them otherwise.
                for ns in node.namespace_iter() {
                    let uri = ns.as_namespace_uri().map_err(engine)?.clone();
                    let prefix = ns.as_namespace_prefix().map_err(engine)?.cloned();
                    copy.add_namespace(
                        copy.new_namespace(uri, prefix, ns.is_in_scope())
                            .map_err(engine)?,
                    )
                    .map_err(engine)?;
                }
                // A union template with a name still declares that name only once.
                if let Some(mut name) = copy.get_attribute_node(&QName::try_from("name").unwrap()) {
                    name.pop().map_err(engine)?;
                }
                copy
            };
            set_attribute(&copy, "match", part)?;
            expanded.push(Declaration {
                node: copy,
                group: declaration.group,
            });
        }
    }
    Ok(expanded)
}

struct Declaration {
    node: IdNode,
    group: usize,
}

struct Assembly {
    // xrust parent/owner links are weak until declarations are attached.
    documents: Vec<IdNode>,
    occurrences: usize,
    bytes: usize,
    precedence: Vec<usize>,
    next_precedence: usize,
}

impl Default for Assembly {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            occurrences: 0,
            bytes: 0,
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
    let declaration_names: Vec<_> = declarations
        .iter()
        .map(|declaration| declaration_name(&declaration.node))
        .collect::<Result<_>>()?;
    let mut bindings = BTreeMap::<String, usize>::new();
    let mut names = BTreeMap::<String, usize>::new();
    let mut outputs = Vec::new();
    let mut priorities = Vec::new();
    for (index, declaration) in declarations.iter().enumerate() {
        let node = &declaration.node;
        let rank = precedence[declaration.group];
        if named(node, "param") || named(node, "variable") {
            let entry = bindings
                .entry(declaration_names[index].clone())
                .or_insert(rank);
            *entry = (*entry).max(rank);
        }
        if named(node, "template") {
            let name = &declaration_names[index];
            if !name.is_empty() {
                let entry = names.entry(name.clone()).or_insert(rank);
                *entry = (*entry).max(rank);
            }
            if !attribute(node, "match").is_empty() {
                let priority = default_priority(node)?;
                priorities.push((index, rank, priority));
            }
        }
        if named(node, "output") && precedence.len() > 1 {
            outputs.push((index, rank));
        }
        if precedence.len() > 1 && contains_apply_imports(node) {
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
            bindings.get(&declaration_names[index]) == Some(&rank)
        } else if named(node, "output") && first_output.is_some() {
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
            let name = declaration_name(node)?;
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

// XSLT 1.0 section 5.5: union alternatives have already become separate rules.
// Validate with xrust, then classify a single pattern, rather than using its
// terminal node test (which loses both path steps and union alternatives).
fn default_priority(node: &IdNode) -> Result<f64> {
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
    let pattern = attribute(node, "match");
    let parsed: xrust::pattern::Pattern<IdNode> =
        xrust::pattern::Pattern::try_from((pattern.clone(), node.clone())).map_err(engine)?;
    if let Some(error) = parsed.get_err() {
        return Err(engine(error));
    }
    let mut quote = None;
    let compact: String = pattern
        .chars()
        .filter(|&ch| {
            if let Some(delimiter) = quote {
                if ch == delimiter {
                    quote = None;
                }
                true
            } else {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                }
                !matches!(ch, ' ' | '\t' | '\n' | '\r')
            }
        })
        .collect();
    let step = compact.as_str();
    let step = step
        .strip_prefix("child::")
        .or_else(|| step.strip_prefix("attribute::"))
        .or_else(|| step.strip_prefix('@'))
        .unwrap_or(step)
        .trim();
    if matches!(
        step,
        "*" | "node()" | "text()" | "comment()" | "processing-instruction()"
    ) {
        return Ok(-0.5);
    }
    if let Some(prefix) = step.strip_suffix(":*") {
        if NcName::try_from(prefix).is_ok() {
            return Ok(-0.25);
        }
    }
    if NcName::try_from(step).is_ok()
        || step.split_once(':').is_some_and(|(prefix, local)| {
            NcName::try_from(prefix).is_ok() && NcName::try_from(local).is_ok()
        })
    {
        return Ok(0.0);
    }
    if let Some(target) = step
        .strip_prefix("processing-instruction(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let target = target.trim();
        if target.len() >= 2
            && ((target.starts_with('\'')
                && target.ends_with('\'')
                && !target[1..target.len() - 1].contains('\''))
                || (target.starts_with('"')
                    && target.ends_with('"')
                    && !target[1..target.len() - 1].contains('"')))
        {
            return Ok(0.0);
        }
    }
    Ok(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_byte_budget_accepts_its_boundary_and_refuses_overflow() {
        let mut bytes = limits::MAX_ASSEMBLED_STYLESHEET_BYTES - 1;
        charge_bytes(&mut bytes, 1).unwrap();
        assert!(matches!(
            charge_bytes(&mut bytes, 1),
            Err(Error::InvalidArgument { .. })
        ));
        assert!(charge_bytes(&mut 1, usize::MAX).is_err());
    }

    #[test]
    fn union_separators_inside_literals_and_predicates_are_not_split() {
        assert_eq!(
            alternatives("a|processing-instruction('a|b')|b"),
            vec!["a", "processing-instruction('a|b')", "b"]
        );
        assert_eq!(alternatives("a[@b='x|y']|@c"), vec!["a[@b='x|y']", "@c"]);
    }
}
