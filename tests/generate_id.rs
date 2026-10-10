//! `generate-id()` answers the same id for the same input in every process (ledger #1047).
//!
//! The claim: xrust 2.2.0's `generate-id()` is `format!("{:#p}", …)` of the node's heap
//! address (`trees/smite.rs`, `get_id`), so a stylesheet calling it through
//! `urn:xslt:transform` returns e.g. `0x00000076eb01c790` — which differs from process to
//! process under ASLR (so a `.cacheable()` answer is not a function of its inputs), tells the
//! caller where the heap is, and is not an NCName, which XSLT requires of it.

use futures::executor::block_on;
use ikigai_core::{ArgRef, Capability, Iri, Kernel, Request, Verb};
use std::process::Command;
use std::sync::Arc;

/// One stylesheet that asks for the id of every kind of node there is, plus the ids of the
/// same nodes reached a second way (to check same node → same id).
const STYLE: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/">
    <xsl:text>root=</xsl:text><xsl:value-of select="generate-id()"/>
    <xsl:for-each select="//node()">
      <xsl:text>;</xsl:text><xsl:value-of select="local-name()"/>=<xsl:value-of select="generate-id()"/>
    </xsl:for-each>
    <xsl:for-each select="//@*">
      <xsl:text>;@</xsl:text><xsl:value-of select="local-name()"/>=<xsl:value-of select="generate-id()"/>
    </xsl:for-each>
    <xsl:text>;again=</xsl:text><xsl:value-of select="generate-id(//b[@c])"/>
    <xsl:text>;none=[</xsl:text><xsl:value-of select="generate-id(//nosuch)"/><xsl:text>]</xsl:text>
  </xsl:template>
</xsl:stylesheet>"#;

const SRC: &str = r#"<doc a="1" z="2"><b c="3">t<!--k--><?p q?></b><b/></doc>"#;

fn transform(src: &str, style: &str) -> String {
    let kernel = Kernel::new(Arc::new(ikigai_xslt::space()));
    let req = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(src.as_bytes().to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(style.as_bytes().to_vec()));
    let rep = block_on(kernel.issue(req, &Capability::root())).expect("transform");
    String::from_utf8(rep.bytes).expect("utf-8")
}

/// The `name=id` pairs of one run, `;`-separated (xrust 2.2.0 drops an `xsl:text` that is
/// only whitespace, so a space cannot separate them): the nodes, then the attributes, each in
/// document order.
fn ids() -> String {
    transform(SRC, STYLE)
}

/// Run by [`the_ids_are_the_same_in_another_process`] in a child process, which is the only
/// place ASLR can move the heap: it prints one marked line and nothing else of interest.
#[test]
#[ignore = "a child of the_ids_are_the_same_in_another_process, not a test by itself"]
fn print_ids_for_the_parent() {
    println!("IDS:{}", ids());
}

fn ids_from_a_child_process() -> String {
    let out = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "print_ids_for_the_parent",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .expect("run the test binary");
    assert!(out.status.success(), "child failed: {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .lines()
        .find_map(|l| l.split_once("IDS:").map(|(_, ids)| ids))
        .unwrap_or_else(|| panic!("no IDS line in the child's output: {stdout}"))
        .to_string()
}

#[test]
fn the_ids_are_the_same_in_another_process() {
    let here = ids();
    let there = ids_from_a_child_process();
    let elsewhere = ids_from_a_child_process();
    assert_eq!(
        here, there,
        "a cacheable answer must not depend on the process"
    );
    assert_eq!(
        there, elsewhere,
        "a cacheable answer must not depend on the process"
    );
}

/// The values themselves, pinned: an id is the node's position in its tree (see the crate
/// documentation), so this is the shape a consumer can rely on, and a change to it is a change
/// to every cached answer that carries one.
#[test]
fn the_ids_are_the_nodes_positions() {
    assert_eq!(
        ids(),
        "root=d1;doc=d1c1;b=d1c1c1;=d1c1c1c1;=d1c1c1c2;p=d1c1c1c3;b=d1c1c2;\
         @a=d1c1a1;@z=d1c1a2;@c=d1c1c1a1;again=d1c1c1;none=[]"
    );
}

/// The `name=id` pairs of one run, without the two that repeat a node or name none.
fn every_node_once(out: &str) -> Vec<&str> {
    out.split(';')
        .filter(|p| !p.starts_with("again=") && !p.starts_with("none="))
        .map(|p| p.split_once('=').expect("name=id").1)
        .collect()
}

/// XSLT 1.0 §12.4: "an ASCII alphanumeric string that must start with an alphabetic
/// character" (and so an NCName too). A heap address, `0x…`, starts with a digit.
#[test]
fn every_id_is_alphanumeric_and_starts_with_a_letter() {
    let out = ids();
    for id in every_node_once(&out) {
        assert!(
            id.starts_with(|c: char| c.is_ascii_alphabetic())
                && id.chars().all(|c| c.is_ascii_alphanumeric()),
            "{id} in {out}"
        );
    }
}

/// Same node, same id; different nodes, different ids: over every node of the source.
#[test]
fn distinct_nodes_have_distinct_ids() {
    let out = ids();
    let mut seen = every_node_once(&out);
    let n = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), n, "two nodes shared an id: {out}");
    assert!(
        out.contains(";b=d1c1c1;") && out.contains(";again=d1c1c1;"),
        "{out}"
    );
}

/// Two runs in one process — a memo hit and a fresh compile — answer the same ids, so nothing
/// a run remembers leaks into the next.
#[test]
fn a_second_run_answers_the_same_ids() {
    let first = ids();
    ikigai_xslt::clear_stylesheet_cache();
    let compiled = ikigai_xslt::CompiledStylesheet::compile(STYLE).expect("compile");
    // A different document first, so a leftover tree number or id would show.
    compiled.transform("<x><y/></x>", true).expect("other");
    assert_eq!(compiled.transform(SRC, true).expect("again"), first);
    assert_eq!(ids(), first);
}
