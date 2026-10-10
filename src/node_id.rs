//! `generate-id()` as a node's position in its tree, not its heap address (ledger #1047).
//!
//! xrust 2.2.0 answers `generate-id()` with `format!("{:#p}", …)` of the node — e.g.
//! `0x00000076eb01c790` — so the same stylesheet over the same document answered a different
//! id in every process (ASLR), which made a `.cacheable()` answer depend on something other
//! than its inputs, told whoever wrote the stylesheet where the heap is, and is not even the
//! "alphanumeric, starting with a letter" string XSLT promises. xrust offers no hook for one
//! function — its compiled transforms are `pub(crate)` — but its whole engine is generic over
//! the [`Node`] trait, and `generate-id()` is exactly `Node::get_id`. So this crate runs xrust
//! over [`IdNode`]: a newtype over xrust's own `RNode` that forwards every method unchanged
//! except that one.
//!
//! ## The id
//!
//! A node's id is the path to it from the top of its tree, in ASCII letters and digits:
//!
//! - the top: `d{k}` for a document node, `u{k}` for the top of a tree that is not in a
//!   document, where `k` numbers the trees of one transform from 1. The **source document is
//!   always `d1`**; any other tree takes the next number the first time an id is asked of it;
//! - each step down: `c{i}` for the `i`-th child (elements, text, comments and processing
//!   instructions share one count), `a{i}` for the `i`-th attribute in xrust's attribute order
//!   (by name, a `BTreeMap`), `n{i}` for the `i`-th namespace node — each counting from 1.
//!
//! ```text
//! <doc a="1"><b>t<!--k--></b></doc>
//!   /       d1          @a   d1c1a1      text      d1c1c1c1
//!   doc     d1c1        b    d1c1c1      comment   d1c1c1c2
//! ```
//!
//! That is the XSLT contract: the same node always gets the same id within a transform (it is
//! remembered on first use, so a tree that changes later cannot move it), two nodes never share
//! one (a letter separates every number, so a path reads back one way, and every tree has its
//! own top), and it is alphanumeric and starts with a letter (XSLT 1.0 §12.4, and so an NCName).
//! It is also a pure function of the input and of the order the stylesheet asks for trees
//! after the first — and xrust evaluates in one deterministic order — so a cached answer is the
//! answer every process gives. It is not stable across *different* source documents: an
//! inserted node renumbers its later siblings, as XSLT allows.
//!
//! ## Its cost
//!
//! Finding a node's position scans its parent's children once; every sibling found on the way
//! is remembered, so asking the id of every node of a document is linear in the document, and
//! the memory is one entry per node asked about (or passed over), freed when the transform
//! ends. The memo lives in a thread-local [`Scope`] that [`crate::CompiledStylesheet`] opens
//! around each run; xrust is single-threaded and each run has its thread to itself.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use qualname::{NamespacePrefix, NamespaceUri, QName};
use xrust::item::{Node, NodeType};
use xrust::output::OutputDefinition;
use xrust::trees::smite::RNode;
use xrust::validators::{Schema, ValidationError};
use xrust::value::Value;
use xrust::xdmerror::Error;
use xrust::xmldecl::{XMLDecl, DTD};

/// xrust's tree, with `generate-id()` answered by position. Every method but
/// [`Node::get_id`] forwards to the `RNode` inside, so the engine sees exactly the tree it
/// would have.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IdNode(RNode);

/// The ids of one transform: what [`IdNode::get_id`] has answered, and the trees seen.
#[derive(Default)]
struct Ids {
    /// The top of every tree seen, numbered from 1 by position here.
    trees: Vec<RNode>,
    /// Node address → (the node, its id). The node is held so that its address cannot be
    /// reused by another node while the transform runs, which is what makes the address a
    /// sound key. Addresses never leave this map.
    known: HashMap<usize, (RNode, Rc<str>)>,
}

thread_local! {
    static IDS: RefCell<Ids> = RefCell::new(Ids::default());
}

/// The address of the node an `RNode` points at: the key [`Ids::known`] is held under.
fn address(n: &RNode) -> usize {
    Rc::as_ptr(n) as *const () as usize
}

impl Ids {
    fn id_of(&mut self, n: &RNode) -> Rc<str> {
        if let Some((_, id)) = self.known.get(&address(n)) {
            return Rc::clone(id);
        }
        let Some(parent) = n.parent() else {
            return self.top(n);
        };
        let base = self.id_of(&parent);
        if n.node_type() == NodeType::Namespace {
            // xrust makes namespace nodes when asked for them, so the same namespace is a new
            // object every time: find it by what it declares, and do not remember the object.
            let i = parent
                .namespace_iter()
                .position(|m| m.name() == n.name() && m.value() == n.value())
                .map_or(0, |i| i + 1);
            return format!("{base}n{i}").into();
        }
        let (step, siblings) = if n.node_type() == NodeType::Attribute {
            ('a', parent.attribute_iter())
        } else {
            ('c', parent.child_iter())
        };
        for (i, sibling) in siblings.enumerate() {
            self.known.entry(address(&sibling)).or_insert_with(|| {
                let id: Rc<str> = format!("{base}{step}{}", i + 1).into();
                (sibling, id)
            });
        }
        match self.known.get(&address(n)) {
            Some((_, id)) => Rc::clone(id),
            // A node whose parent does not list it (xrust keeps a parent link on some nodes it
            // has not attached): it is the top of a tree of its own.
            None => self.top(n),
        }
    }

    /// The id of a node nothing is above: its tree's number, given on first sight.
    fn top(&mut self, n: &RNode) -> Rc<str> {
        let k = match self.trees.iter().position(|t| Rc::ptr_eq(t, n)) {
            Some(i) => i + 1,
            None => {
                self.trees.push(n.clone());
                self.trees.len()
            }
        };
        let letter = if n.node_type() == NodeType::Document {
            'd'
        } else {
            'u'
        };
        let id: Rc<str> = format!("{letter}{k}").into();
        self.known.insert(address(n), (n.clone(), Rc::clone(&id)));
        id
    }
}

/// The ids of one transform, from the moment it is opened until it is dropped — on a panic
/// too, so nothing a run asked about outlives it on a pooled thread.
pub(crate) struct Scope(());

impl Scope {
    /// Start a transform's ids with `source` as tree 1 (`d1`).
    pub(crate) fn open(source: &IdNode) -> Self {
        IDS.with(|ids| {
            let mut ids = ids.borrow_mut();
            *ids = Ids::default();
            ids.top(&source.0);
        });
        Scope(())
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        // `try_with`: a scope dropped while the thread itself is being torn down has
        // nothing left to clear.
        let _ = IDS.try_with(|ids| *ids.borrow_mut() = Ids::default());
    }
}

/// An `RNode` iterator, re-wrapped.
fn wrap(it: Box<dyn Iterator<Item = RNode>>) -> Box<dyn Iterator<Item = IdNode>> {
    Box::new(it.map(IdNode))
}

impl Node for IdNode {
    type NodeIterator = Box<dyn Iterator<Item = IdNode>>;

    /// The one method that is not forwarded. See the module documentation.
    fn get_id(&self) -> String {
        IDS.with(|ids| ids.borrow_mut().id_of(&self.0)).to_string()
    }

    fn new_document() -> Self {
        IdNode(RNode::new_document())
    }
    fn node_type(&self) -> NodeType {
        self.0.node_type()
    }
    fn name(&self) -> Option<QName> {
        self.0.name()
    }
    fn value(&self) -> Rc<Value> {
        self.0.value()
    }
    fn to_qname(&self, name: impl AsRef<str>) -> Result<QName, Error> {
        self.0.to_qname(name)
    }
    fn to_prefixed_name(&self) -> String {
        self.0.to_prefixed_name()
    }
    fn to_namespace_prefix(&self, nsuri: &NamespaceUri) -> Result<Option<NamespacePrefix>, Error> {
        self.0.to_namespace_prefix(nsuri)
    }
    fn to_namespace_uri(&self, prefix: &Option<NamespacePrefix>) -> Result<NamespaceUri, Error> {
        self.0.to_namespace_uri(prefix)
    }
    fn as_namespace_prefix(&self) -> Result<Option<&NamespacePrefix>, Error> {
        self.0.as_namespace_prefix()
    }
    fn as_namespace_uri(&self) -> Result<&NamespaceUri, Error> {
        self.0.as_namespace_uri()
    }
    fn is_in_scope(&self) -> bool {
        self.0.is_in_scope()
    }
    fn to_string(&self) -> String {
        Node::to_string(&self.0)
    }
    fn to_xml(&self) -> String {
        self.0.to_xml()
    }
    fn to_xml_with_options(&self, od: &OutputDefinition) -> String {
        self.0.to_xml_with_options(od)
    }
    fn to_json(&self) -> String {
        self.0.to_json()
    }
    fn is_same(&self, other: &Self) -> bool {
        self.0.is_same(&other.0)
    }
    fn is_attached(&self) -> bool {
        self.0.is_attached()
    }
    fn document_order(&self) -> Vec<usize> {
        self.0.document_order()
    }
    fn cmp_document_order(&self, other: &Self) -> Ordering {
        self.0.cmp_document_order(&other.0)
    }
    fn is_element(&self) -> bool {
        self.0.is_element()
    }
    fn is_unattached(&self) -> bool {
        self.0.is_unattached()
    }
    fn is_id(&self) -> bool {
        self.0.is_id()
    }
    fn is_idrefs(&self) -> bool {
        self.0.is_idrefs()
    }
    fn child_iter(&self) -> Self::NodeIterator {
        wrap(self.0.child_iter())
    }
    fn first_child(&self) -> Option<Self> {
        self.0.first_child().map(IdNode)
    }
    fn ancestor_iter(&self) -> Self::NodeIterator {
        wrap(self.0.ancestor_iter())
    }
    fn parent(&self) -> Option<Self> {
        self.0.parent().map(IdNode)
    }
    fn owner_document(&self) -> Self {
        IdNode(self.0.owner_document())
    }
    fn descend_iter(&self) -> Self::NodeIterator {
        wrap(self.0.descend_iter())
    }
    fn next_iter(&self) -> Self::NodeIterator {
        wrap(self.0.next_iter())
    }
    fn prev_iter(&self) -> Self::NodeIterator {
        wrap(self.0.prev_iter())
    }
    fn attribute_iter(&self) -> Self::NodeIterator {
        wrap(self.0.attribute_iter())
    }
    fn get_attribute(&self, a: &QName) -> Rc<Value> {
        self.0.get_attribute(a)
    }
    fn get_attribute_node(&self, a: &QName) -> Option<Self> {
        self.0.get_attribute_node(a).map(IdNode)
    }
    fn new_element(&self, qn: QName) -> Result<Self, Error> {
        self.0.new_element(qn).map(IdNode)
    }
    fn new_text(&self, v: Rc<Value>) -> Result<Self, Error> {
        self.0.new_text(v).map(IdNode)
    }
    fn new_attribute(&self, qn: QName, v: Rc<Value>) -> Result<Self, Error> {
        self.0.new_attribute(qn, v).map(IdNode)
    }
    fn new_comment(&self, v: Rc<Value>) -> Result<Self, Error> {
        self.0.new_comment(v).map(IdNode)
    }
    fn new_processing_instruction(&self, qn: Rc<Value>, v: Rc<Value>) -> Result<Self, Error> {
        self.0.new_processing_instruction(qn, v).map(IdNode)
    }
    fn new_namespace(
        &self,
        ns: NamespaceUri,
        prefix: Option<NamespacePrefix>,
        in_scope: bool,
    ) -> Result<Self, Error> {
        self.0.new_namespace(ns, prefix, in_scope).map(IdNode)
    }
    fn push(&mut self, n: Self) -> Result<(), Error> {
        self.0.push(n.0)
    }
    fn pop(&mut self) -> Result<(), Error> {
        self.0.pop()
    }
    fn insert_before(&mut self, n: Self) -> Result<(), Error> {
        self.0.insert_before(n.0)
    }
    fn add_attribute(&self, att: Self) -> Result<(), Error> {
        self.0.add_attribute(att.0)
    }
    fn shallow_copy(&self) -> Result<Self, Error> {
        self.0.shallow_copy().map(IdNode)
    }
    fn deep_copy(&self) -> Result<Self, Error> {
        self.0.deep_copy().map(IdNode)
    }
    fn get_canonical(&self) -> Result<Self, Error> {
        self.0.get_canonical().map(IdNode)
    }
    fn xmldecl(&self) -> XMLDecl {
        self.0.xmldecl()
    }
    fn set_xmldecl(&mut self, d: XMLDecl) -> Result<(), Error> {
        self.0.set_xmldecl(d)
    }
    fn add_namespace(&self, ns: Self) -> Result<(), Error> {
        self.0.add_namespace(ns.0)
    }
    fn eq(&self, other: &Self) -> bool {
        Node::eq(&self.0, &other.0)
    }
    fn namespace_iter(&self) -> Self::NodeIterator {
        wrap(self.0.namespace_iter())
    }
    fn get_dtd(&self) -> Option<DTD> {
        self.0.get_dtd()
    }
    fn set_dtd(&self, dtd: DTD) -> Result<(), Error> {
        self.0.set_dtd(dtd)
    }
    fn validate(&self, schema: Schema) -> Result<(), ValidationError> {
        self.0.validate(schema)
    }
    fn unattached(&self) -> Vec<Self> {
        self.0.unattached().into_iter().map(IdNode).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_xml;

    /// Every node of a document, in document order, with attributes after their element.
    fn all_nodes(n: &IdNode, out: &mut Vec<IdNode>) {
        out.push(n.clone());
        out.extend(n.attribute_iter());
        for c in n.child_iter() {
            all_nodes(&c, out);
        }
    }

    #[test]
    fn ids_are_positions_and_distinct() {
        let doc = parse_xml(r#"<doc a="1" z="2"><b c="3">t<!--k--><?p q?></b><b/></doc>"#)
            .expect("parse");
        let _scope = Scope::open(&doc);
        let mut nodes = vec![];
        all_nodes(&doc, &mut nodes);
        let ids: Vec<String> = nodes.iter().map(|n| n.get_id()).collect();
        assert_eq!(
            ids,
            [
                "d1", "d1c1", "d1c1a1", "d1c1a2", "d1c1c1", "d1c1c1a1", "d1c1c1c1", "d1c1c1c2",
                "d1c1c1c3", "d1c1c2"
            ]
        );
        // Asked again, in reverse, through fresh handles: the same answers.
        let mut again = vec![];
        all_nodes(&doc, &mut again);
        let again: Vec<String> = again.iter().rev().map(|n| n.get_id()).collect();
        assert_eq!(again.into_iter().rev().collect::<Vec<_>>(), ids);
    }

    #[test]
    fn a_second_tree_and_a_detached_node_get_their_own_tops() {
        let doc = parse_xml("<a><b/></a>").expect("parse");
        let other = parse_xml("<a><b/></a>").expect("parse");
        let _scope = Scope::open(&doc);
        let b_other = other
            .child_iter()
            .next()
            .unwrap()
            .child_iter()
            .next()
            .unwrap();
        assert_eq!(b_other.get_id(), "d2c1c1");
        let b_doc = doc
            .child_iter()
            .next()
            .unwrap()
            .child_iter()
            .next()
            .unwrap();
        assert_eq!(b_doc.get_id(), "d1c1c1");
        let loose = doc
            .new_element(QName::from_local_name(
                qualname::NcName::try_from("loose").unwrap(),
            ))
            .expect("element");
        assert_eq!(loose.get_id(), "u3");
        assert_eq!(loose.get_id(), "u3", "the same node keeps its id");
    }

    #[test]
    fn a_scope_forgets_everything_when_it_ends() {
        let doc = parse_xml("<a/>").expect("parse");
        {
            let _scope = Scope::open(&doc);
            doc.child_iter().next().unwrap().get_id();
            assert!(IDS.with(|ids| !ids.borrow().known.is_empty()));
        }
        assert!(IDS.with(|ids| ids.borrow().known.is_empty() && ids.borrow().trees.is_empty()));
    }
}
