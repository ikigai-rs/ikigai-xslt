use ikigai_core::{
    ArgRef, Capability, Exact, FnEndpoint, Invocation, Iri, Kernel, ReprType, Representation,
    Request, Verb,
};
use std::sync::Arc;

fn style(body: &str) -> String {
    format!("<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform' version='1.0'>{body}</xsl:stylesheet>")
}

#[test]
fn an_absolute_import_reaches_its_module() {
    let module = style("<xsl:template match='/'><answer>imported</answer></xsl:template>");
    let space = ikigai_xslt::space().bind(
        Exact::new("urn:file:module.xsl"),
        FnEndpoint::new("module", move |_: &Invocation<'_>| {
            Ok(
                Representation::new(ReprType::new("application/xml"), module.as_bytes().to_vec())
                    .cacheable(),
            )
        }),
    );
    let kernel = Kernel::new(Arc::new(space));
    let request = Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc/>".to_vec()))
        .with_arg(
            "stylesheet",
            ArgRef::Inline(style("<xsl:import href='urn:file:module.xsl'/>").into_bytes()),
        );
    let result = futures::executor::block_on(kernel.issue(request, &Capability::root()))
        .expect("imported stylesheet transforms");
    assert_eq!(
        String::from_utf8(result.bytes).unwrap(),
        "<answer>imported</answer>"
    );
}

use ikigai_core::{Description, EndpointSpace, Error};
use std::collections::BTreeMap;
use std::sync::RwLock;

fn request(stylesheet: &str) -> Request {
    Request::new(Verb::Source, Iri::parse("urn:xslt:transform").unwrap())
        .with_arg("src", ArgRef::Inline(b"<doc><item>a</item></doc>".to_vec()))
        .with_arg("stylesheet", ArgRef::Inline(stylesheet.as_bytes().to_vec()))
}

fn issue(kernel: &Kernel, stylesheet: &str) -> ikigai_core::Result<Representation> {
    futures::executor::block_on(kernel.issue(request(stylesheet), &Capability::root()))
}

fn fixed(space: EndpointSpace, iri: &str, text: String) -> EndpointSpace {
    let thread = iri.to_string();
    space.bind(
        Exact::new(iri),
        FnEndpoint::new("stylesheet-file", move |_: &Invocation<'_>| {
            Ok(
                Representation::new(ReprType::new("application/xml"), text.as_bytes().to_vec())
                    .cacheable()
                    .depends_on(thread.clone()),
            )
        }),
    )
}

fn kernel(files: &[(&str, String)]) -> Kernel {
    let space = files
        .iter()
        .fold(ikigai_xslt::space(), |space, (iri, text)| {
            fixed(space, iri, text.clone())
        });
    Kernel::new(Arc::new(space))
}

fn answer(value: &str) -> String {
    style(&format!(
        "<xsl:template match='/'><answer>{value}</answer></xsl:template>"
    ))
}

#[test]
fn urn_relative_parent_and_absolute_hrefs_and_include() {
    for (kind, href, target) in [
        ("import", "shared.xsl", "urn:file:/site/styles/shared.xsl"),
        (
            "import",
            "../common/base.xsl",
            "urn:file:/site/common/base.xsl",
        ),
        ("import", "urn:style:absolute", "urn:style:absolute"),
        ("include", "shared.xsl", "urn:file:/site/styles/shared.xsl"),
    ] {
        let main = style(&format!("<xsl:{kind} href='{href}'/>"));
        let kernel = kernel(&[
            ("urn:file:/site/styles/main.xsl", main),
            (target, answer("loaded")),
        ]);
        let result = issue(&kernel, "urn:file:/site/styles/main.xsl").unwrap();
        assert_eq!(
            String::from_utf8(result.bytes).unwrap(),
            "<answer>loaded</answer>"
        );
    }
}

#[test]
fn nested_imports_and_includes_resolve_against_their_own_module() {
    let kernel = kernel(&[
        (
            "urn:file:/site/main.xsl",
            style("<xsl:import href='lib/first.xsl'/>"),
        ),
        (
            "urn:file:/site/lib/first.xsl",
            style("<xsl:include href='nested/second.xsl'/>"),
        ),
        (
            "urn:file:/site/lib/nested/second.xsl",
            style("<xsl:import href='../third.xsl'/>"),
        ),
        ("urn:file:/site/lib/third.xsl", answer("nested")),
    ]);
    let result = issue(&kernel, "urn:file:/site/main.xsl").unwrap();
    assert_eq!(
        String::from_utf8(result.bytes).unwrap(),
        "<answer>nested</answer>"
    );
}

#[test]
fn identical_relative_hrefs_in_different_modules_do_not_collide() {
    let main = style("<xsl:import href='left/main.xsl'/><xsl:import href='right/main.xsl'/><xsl:template match='/'><xsl:call-template name='left'/><xsl:call-template name='right'/></xsl:template>");
    let kernel = kernel(&[
        ("urn:file:/site/main.xsl", main),
        (
            "urn:file:/site/left/main.xsl",
            style("<xsl:include href='shared.xsl'/>"),
        ),
        (
            "urn:file:/site/right/main.xsl",
            style("<xsl:include href='shared.xsl'/>"),
        ),
        (
            "urn:file:/site/left/shared.xsl",
            style("<xsl:template name='left'>L</xsl:template>"),
        ),
        (
            "urn:file:/site/right/shared.xsl",
            style("<xsl:template name='right'>R</xsl:template>"),
        ),
    ]);
    let result = issue(&kernel, "urn:file:/site/main.xsl").unwrap();
    assert_eq!(String::from_utf8(result.bytes).unwrap(), "LR");
}

#[test]
fn cycles_name_the_whole_href_chain() {
    let kernel = kernel(&[
        ("urn:file:/main.xsl", style("<xsl:import href='a.xsl'/>")),
        ("urn:file:/a.xsl", style("<xsl:include href='b.xsl'/>")),
        ("urn:file:/b.xsl", style("<xsl:import href='a.xsl'/>")),
    ]);
    let error = issue(&kernel, "urn:file:/main.xsl").unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("cycle") && detail.contains("urn:file:/main.xsl -> urn:file:/a.xsl -> urn:file:/b.xsl -> urn:file:/a.xsl")),
        "{error:?}"
    );
}

#[test]
fn deep_module_chains_are_refused() {
    let files: Vec<_> = (0..18)
        .map(|i| {
            (
                format!("urn:file:/s{i}.xsl"),
                style(&format!("<xsl:import href='s{}.xsl'/>", i + 1)),
            )
        })
        .collect();
    let files: Vec<_> = files
        .iter()
        .map(|(iri, text)| (iri.as_str(), text.clone()))
        .collect();
    let error = issue(&kernel(&files), "urn:file:/s0.xsl").unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { detail, .. } if detail.contains("depth 16") && detail.contains("urn:file:/s17.xsl")),
        "{error:?}"
    );
}

#[test]
fn an_inline_stylesheet_refuses_a_relative_href_and_accepts_an_absolute_one() {
    let kernel = kernel(&[("urn:file:module.xsl", answer("absolute"))]);
    let error = issue(&kernel, &style("<xsl:include href='relative.xsl'/>")).unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("relative.xsl") && detail.contains("no base IRI")),
        "{error:?}"
    );
    assert!(issue(&kernel, &style("<xsl:include href='urn:file:module.xsl'/>")).is_ok());
}

#[test]
fn missing_modules_are_not_found_and_name_the_href() {
    let kernel = kernel(&[(
        "urn:file:/main.xsl",
        style("<xsl:import href='missing.xsl'/>"),
    )]);
    let error = issue(&kernel, "urn:file:/main.xsl").unwrap_err();
    assert!(
        matches!(&error, Error::NotFound(detail) if detail.contains("missing.xsl") && detail.contains("urn:file:/missing.xsl")),
        "{error:?}"
    );
}

#[test]
fn module_capabilities_are_enforced_by_the_kernel() {
    let secret = FnEndpoint::new("secret-module", |_: &Invocation<'_>| {
        Ok(Representation::new(
            ReprType::new("application/xml"),
            answer("secret").into_bytes(),
        )
        .cacheable())
    })
    .with_description(
        Description::new("secret-module")
            .verb(Verb::Source)
            .requires("urn:cap:secret:*")
            .output("application/xml"),
    );
    let kernel = Kernel::new(Arc::new(
        ikigai_xslt::space().bind(Exact::new("urn:secret:style"), secret),
    ));
    let stylesheet = style("<xsl:import href='urn:secret:style'/>");
    let error = futures::executor::block_on(kernel.issue(
        request(&stylesheet),
        &Capability::scoped(Vec::<String>::new()),
    ))
    .unwrap_err();
    assert!(matches!(error, Error::Denied(_)), "{error:?}");
    assert!(issue(&kernel, &stylesheet).is_ok());
}

#[test]
fn cutting_a_nested_module_recomputes_and_recompiles_the_transform() {
    let changed = Arc::new(RwLock::new(answer("before")));
    let store = Arc::clone(&changed);
    let leaf = FnEndpoint::new("leaf-module", move |_: &Invocation<'_>| {
        Ok(Representation::new(
            ReprType::new("application/xml"),
            store.read().unwrap().as_bytes().to_vec(),
        )
        .cacheable()
        .depends_on("urn:file:/leaf.xsl"))
    });
    let space = fixed(
        ikigai_xslt::space(),
        "urn:file:/main.xsl",
        style("<xsl:import href='middle.xsl'/>"),
    );
    let space = fixed(
        space,
        "urn:file:/middle.xsl",
        style("<xsl:include href='leaf.xsl'/>"),
    );
    let kernel = Kernel::new(Arc::new(space.bind(Exact::new("urn:file:/leaf.xsl"), leaf)));
    let first = issue(&kernel, "urn:file:/main.xsl").unwrap();
    let threads: Vec<String> = first.threads().iter().map(ToString::to_string).collect();
    for iri in [
        "urn:file:/main.xsl",
        "urn:file:/middle.xsl",
        "urn:file:/leaf.xsl",
    ] {
        assert!(threads.iter().any(|thread| thread == iri), "{threads:?}");
    }
    *changed.write().unwrap() = answer("after");
    assert_eq!(
        issue(&kernel, "urn:file:/main.xsl").unwrap().bytes,
        first.bytes
    );
    kernel.cut("urn:file:/leaf.xsl");
    let second = issue(&kernel, "urn:file:/main.xsl").unwrap();
    assert_eq!(
        String::from_utf8(second.bytes).unwrap(),
        "<answer>after</answer>"
    );
    assert_eq!(
        String::from_utf8(first.bytes).unwrap(),
        "<answer>before</answer>"
    );
}

#[test]
fn pure_entry_point_is_io_free_and_its_cache_includes_module_bytes() {
    let main = style("<xsl:import href='urn:file:/module.xsl'/>");
    let nested = style("<xsl:include href='leaf.xsl'/>");
    let mut modules = BTreeMap::from([
        ("urn:file:/module.xsl".into(), nested),
        ("urn:file:/leaf.xsl".into(), answer("first")),
    ]);
    assert_eq!(
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap(),
        "first"
    );
    modules.insert("urn:file:/leaf.xsl".into(), answer("second"));
    assert_eq!(
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap(),
        "second"
    );
    modules.remove("urn:file:/leaf.xsl");
    let error =
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap_err();
    assert!(
        error.contains("not found") && error.contains("leaf.xsl"),
        "{error}"
    );
}

#[test]
fn imported_output_parameters_and_adaptations_are_not_skipped() {
    let module = style("<xsl:output method='text'/><xsl:param name='label' select=\"'default'\"/><xsl:template match='/'><xsl:value-of select='$label'/>:<xsl:value-of select='number(doc/item)'/></xsl:template>");
    let kernel = kernel(&[("urn:file:module.xsl", module)]);
    let req = request(&style("<xsl:include href='urn:file:module.xsl'/>"))
        .with_arg("label", ArgRef::Inline(b"supplied".to_vec()));
    let result = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert_eq!(result.repr_type.media_type, "text/plain");
    assert_eq!(String::from_utf8(result.bytes).unwrap(), "supplied:NaN");
}

#[test]
fn imported_subset_refusals_are_still_typed() {
    let module = style("<xsl:template match='item[@a]'>bad</xsl:template>");
    let kernel = kernel(&[("urn:file:module.xsl", module)]);
    let error = issue(&kernel, &style("<xsl:import href='urn:file:module.xsl'/>")).unwrap_err();
    assert!(matches!(error, Error::InvalidArgument { .. }), "{error:?}");
}

#[test]
fn import_precedence_beats_priority_and_later_imports_beat_earlier_ones() {
    let low = style("<xsl:template match='item' priority='999'>low</xsl:template>");
    let middle = style("<xsl:template match='item' priority='-999'>middle</xsl:template>");
    let main = style("<xsl:import href='urn:file:low'/><xsl:import href='urn:file:middle'/><xsl:template match='/'><xsl:apply-templates select='doc/item'/></xsl:template>");
    let kernel = kernel(&[("urn:file:low", low), ("urn:file:middle", middle)]);
    assert_eq!(
        String::from_utf8(issue(&kernel, &main).unwrap().bytes).unwrap(),
        "middle"
    );
    let main = style("<xsl:import href='urn:file:low'/><xsl:template match='/'><xsl:apply-templates select='doc/item'/></xsl:template><xsl:template match='item' priority='-1000'>main</xsl:template>");
    assert_eq!(
        String::from_utf8(issue(&kernel, &main).unwrap().bytes).unwrap(),
        "main"
    );
}

#[test]
fn main_declarations_override_imported_defaults() {
    let imported = style("<xsl:output method='xml'/><xsl:param name='label' select=\"'imported'\"/><xsl:template name='name'>imported</xsl:template>");
    let main = style("<xsl:import href='urn:file:defaults'/><xsl:output method='text'/><xsl:param name='label' select=\"'main'\"/><xsl:template name='name'>main</xsl:template><xsl:template match='/'><xsl:value-of select='$label'/>:<xsl:call-template name='name'/></xsl:template>");
    let result = issue(&kernel(&[("urn:file:defaults", imported)]), &main).unwrap();
    assert_eq!(result.repr_type.media_type, "text/plain");
    assert_eq!(String::from_utf8(result.bytes).unwrap(), "main:main");
}

#[test]
fn an_import_keeps_its_xpath_namespace_bindings() {
    let module = "<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform' xmlns:d='urn:data'><xsl:output method='text'/><xsl:template match='/'><xsl:value-of select='doc/d:item'/></xsl:template></xsl:stylesheet>";
    let kernel = kernel(&[("urn:file:module", module.into())]);
    let req = request(&style("<xsl:import href='urn:file:module'/>")).with_arg(
        "src",
        ArgRef::Inline(b"<doc xmlns:d='urn:data'><d:item>namespaced</d:item></doc>".to_vec()),
    );
    let result = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert_eq!(String::from_utf8(result.bytes).unwrap(), "namespaced");
}

#[test]
fn a_shared_module_is_not_a_cycle() {
    let main = style("<xsl:import href='urn:file:/left'/><xsl:import href='urn:file:/right'/>");
    let modules = BTreeMap::from([
        (
            "urn:file:/left".into(),
            style("<xsl:import href='shared'/>"),
        ),
        (
            "urn:file:/right".into(),
            style("<xsl:import href='shared'/>"),
        ),
        ("urn:file:/shared".into(), answer("shared")),
    ]);
    assert_eq!(
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap(),
        "shared"
    );
}

#[test]
fn pure_module_cycles_and_repeated_expansion_are_bounded() {
    let main = style("<xsl:import href='urn:file:/loop'/>");
    let modules = BTreeMap::from([("urn:file:/loop".into(), style("<xsl:include href='loop'/>"))]);
    let error =
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap_err();
    assert!(
        error.contains("cycle") && error.contains("urn:file:/loop -> urn:file:/loop"),
        "{error}"
    );
    let mut modules = BTreeMap::new();
    for i in 0..8 {
        let next = format!("s{}", i + 1);
        modules.insert(
            format!("urn:file:/s{i}"),
            style(&format!(
                "<xsl:include href='{next}'/><xsl:include href='{next}'/>"
            )),
        );
    }
    modules.insert("urn:file:/s8".into(), answer("leaf"));
    let main = style("<xsl:include href='urn:file:/s0'/>");
    let error =
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap_err();
    assert!(error.contains("128 module occurrences"), "{error}");
}

#[test]
fn imported_apply_imports_is_refused_rather_than_answered_wrongly() {
    let main = style("<xsl:import href='urn:file:base'/><xsl:template match='/'><xsl:apply-imports/></xsl:template>");
    let error = issue(&kernel(&[("urn:file:base", answer("base"))]), &main).unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { detail, .. } if detail.contains("xsl:apply-imports")),
        "{error:?}"
    );
}

#[test]
fn module_xml_bounds_apply_before_the_engine_parses_it() {
    let deep = format!("{}{}", "<a>".repeat(65), "</a>".repeat(65));
    let main = style("<xsl:include href='urn:file:deep'/>");
    let error = issue(&kernel(&[("urn:file:deep", deep)]), &main).unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("64")),
        "{error:?}"
    );
}

#[test]
fn only_top_level_xslt_namespace_directives_are_fetched() {
    let stylesheet = "<s:stylesheet xmlns:s='http://www.w3.org/1999/XSL/Transform'><s:template match='/'><include href='not-a-module'/></s:template></s:stylesheet>";
    let result = issue(&kernel(&[]), stylesheet).unwrap();
    assert!(String::from_utf8(result.bytes)
        .unwrap()
        .contains("not-a-module"));
    let main = "<s:stylesheet xmlns:s='http://www.w3.org/1999/XSL/Transform'><s:include href='urn:file:alternate-prefix'/></s:stylesheet>";
    assert!(issue(
        &kernel(&[("urn:file:alternate-prefix", answer("alternate"))]),
        main
    )
    .is_ok());
}

#[test]
fn a_module_directive_requires_a_nonempty_href() {
    for directive in ["<xsl:import/>", "<xsl:include href=''/>"] {
        let error = issue(&kernel(&[]), &style(directive)).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("href")),
            "{error:?}"
        );
    }
}

#[test]
fn a_stylesheet_passed_by_reference_keeps_its_base_iri() {
    let kernel = kernel(&[
        (
            "urn:file:/site/main.xsl",
            style("<xsl:include href='shared.xsl'/>"),
        ),
        ("urn:file:/site/shared.xsl", answer("referenced")),
    ]);
    let req = request("").with_arg(
        "stylesheet",
        ArgRef::Reference(Iri::parse("urn:file:/site/main.xsl").unwrap()),
    );
    let result = futures::executor::block_on(kernel.issue(req, &Capability::root())).unwrap();
    assert_eq!(
        String::from_utf8(result.bytes).unwrap(),
        "<answer>referenced</answer>"
    );
}

#[test]
fn repeated_modules_have_a_byte_budget() {
    let main = style(&"<xsl:include href='urn:file:large'/>".repeat(128));
    let large = style(&format!("<!--{}-->", "x".repeat(70_000)));
    let modules = BTreeMap::from([("urn:file:large".into(), large)]);
    let error =
        ikigai_xslt::transform_xml_with_modules("<doc/>", &main, &modules, true).unwrap_err();
    assert!(
        error.contains("assembled stylesheet") && error.contains("bytes"),
        "{error}"
    );
}

#[test]
fn union_alternatives_have_independent_default_priorities() {
    let modules = BTreeMap::from([("urn:file:base".into(), style(""))]);
    for imports in ["", "<xsl:import href='urn:file:base'/>"] {
        for pattern in ["item|@b", "@b|item"] {
            let main = style(&format!("{imports}<xsl:template match='/'><xsl:apply-templates select='doc/item|doc/@b'/></xsl:template><xsl:template match='{pattern}'>union</xsl:template><xsl:template match='*' priority='-0.25'>element</xsl:template><xsl:template match='@*' priority='-0.25'>attribute</xsl:template>"));
            assert_eq!(
                ikigai_xslt::transform_xml_with_modules(
                    "<doc b='v'><item/></doc>",
                    &main,
                    &modules,
                    true
                )
                .unwrap(),
                "unionunion",
                "{imports} {pattern}"
            );
        }
    }
}

#[test]
fn imports_must_precede_other_declarations_in_each_module() {
    for body in [
        "<xsl:template match='/'>main</xsl:template><xsl:import href='urn:file:base'/>",
        "<xsl:include href='urn:file:empty'/><xsl:import href='urn:file:base'/>",
    ] {
        let main = style(body);
        let kernel = kernel(&[
            ("urn:file:base", answer("base")),
            ("urn:file:empty", style("")),
        ]);
        let error = issue(&kernel, &main).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("xsl:import") && detail.contains("precede")),
            "{error:?}"
        );
    }
}

#[test]
fn duplicate_names_at_the_same_precedence_are_refused() {
    for declarations in [
        "<xsl:template name='same'>one</xsl:template><xsl:template name='same'>two</xsl:template>",
        "<xsl:variable name='same' select='1'/><xsl:variable name='same' select='2'/>",
        "<xsl:param name='same' select='1'/><xsl:variable name='same' select='2'/>",
    ] {
        for prefix in ["", "<xsl:import href='urn:file:empty'/>"] {
            let main = style(&format!(
                "{prefix}{declarations}<xsl:template match='/'>ok</xsl:template>"
            ));
            let error = issue(&kernel(&[("urn:file:empty", style(""))]), &main).unwrap_err();
            assert!(
                matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("duplicate") && detail.contains("same")),
                "{error:?}"
            );
        }
    }
}

#[test]
fn union_wildcards_and_names_keep_different_priorities() {
    let modules = BTreeMap::from([("urn:file:base".into(), style(""))]);
    for imports in ["", "<xsl:import href='urn:file:base'/>"] {
        for (pattern, selected, expected) in [
            ("*|@b", "doc/item", "competing"),
            ("*|@b", "doc/@b", "union"),
            ("@b|*", "doc/item", "competing"),
            ("@b|*", "doc/@b", "union"),
            ("item|@*", "doc/item", "union"),
            ("item|@*", "doc/@b", "competing"),
        ] {
            let main = style(&format!("{imports}<xsl:template match='/'><xsl:apply-templates select='{selected}'/></xsl:template><xsl:template match='{pattern}'>union</xsl:template><xsl:template match='node()|@*' priority='-0.25'>competing</xsl:template>"));
            assert_eq!(
                ikigai_xslt::transform_xml_with_modules(
                    "<doc b='v'><item/></doc>",
                    &main,
                    &modules,
                    true
                )
                .unwrap(),
                expected,
                "{imports} {pattern} {selected}"
            );
        }
    }
}

#[test]
fn split_union_templates_preserve_names_modes_namespaces_and_body_attributes() {
    let module = "<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform' xmlns:d='urn:data'><xsl:template name='both' match='d:a|d:b' mode='special'><xsl:value-of select='name()'/></xsl:template></xsl:stylesheet>";
    let main = style("<xsl:import href='urn:file:union'/><xsl:template match='/'><xsl:call-template name='both'/><xsl:apply-templates select='doc/*' mode='special'/></xsl:template>");
    let modules = BTreeMap::from([("urn:file:union".into(), module.into())]);
    assert_eq!(
        ikigai_xslt::transform_xml_with_modules(
            "<doc xmlns:d='urn:data'><d:a/><d:b/></doc>",
            &main,
            &modules,
            true
        )
        .unwrap(),
        "d:ad:b"
    );
}

#[test]
fn includes_share_their_parents_name_precedence() {
    for (local, included) in [
        (
            "<xsl:template name='same'>main</xsl:template>",
            "<xsl:template name='same'>included</xsl:template>",
        ),
        (
            "<xsl:variable name='same' select='1'/>",
            "<xsl:param name='same' select='2'/>",
        ),
    ] {
        let main = style(&format!("<xsl:include href='urn:file:included'/>{local}<xsl:template match='/'>ok</xsl:template>"));
        let error = issue(&kernel(&[("urn:file:included", style(included))]), &main).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("duplicate") && detail.contains("same")),
            "{error:?}"
        );
    }
}

#[test]
fn duplicate_named_templates_are_compared_by_expanded_qname() {
    let main = "<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform' xmlns:a='urn:names' xmlns:b='urn:names'><xsl:template name='a:same'>a</xsl:template><xsl:template name='b:same'>b</xsl:template><xsl:template match='/'>ok</xsl:template></xsl:stylesheet>";
    let error = issue(&kernel(&[]), main).unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { detail, .. } if detail.contains("duplicate") && detail.contains("{urn:names}same")),
        "{error:?}"
    );
}

#[test]
fn nested_module_import_order_is_checked_but_included_imports_are_hoisted() {
    let main =
        style("<xsl:template match='/'>main</xsl:template><xsl:include href='urn:file:included'/>");
    let kernel = kernel(&[
        (
            "urn:file:included",
            style("<!--allowed before import--><xsl:import href='urn:file:base'/>"),
        ),
        ("urn:file:base", answer("base")),
    ]);
    assert_eq!(
        String::from_utf8(issue(&kernel, &main).unwrap().bytes).unwrap(),
        "main"
    );
    let bad = style("<xsl:template name='early'/><xsl:import href='urn:file:base'/>");
    let main = style("<xsl:include href='urn:file:bad'/>");
    let error = issue(
        &self::kernel(&[("urn:file:bad", bad), ("urn:file:base", answer("base"))]),
        &main,
    )
    .unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { detail, .. } if detail.contains("xsl:import") && detail.contains("precede")),
        "{error:?}"
    );
}

#[test]
fn the_endpoint_refuses_module_bytes_with_a_typed_actionable_sentence() {
    let main = style(&"<xsl:include href='urn:file:large'/>".repeat(128));
    let large = style(&format!("<!--{}-->", "x".repeat(70_000)));
    let error = issue(&kernel(&[("urn:file:large", large)]), &main).unwrap_err();
    assert!(
        matches!(&error, Error::InvalidArgument { name, detail } if name == "stylesheet" && detail.contains("8388608 bytes") && detail.contains("reduce") && detail.ends_with('.')),
        "{error:?}"
    );
    let main = style(&format!(
        "{}<xsl:template match='/'>small</xsl:template>",
        "<xsl:include href='urn:file:small'/>".repeat(128)
    ));
    assert_eq!(
        String::from_utf8(
            issue(&kernel(&[("urn:file:small", style("<!--small-->"))]), &main)
                .unwrap()
                .bytes
        )
        .unwrap(),
        "small"
    );
}

#[test]
fn output_media_type_follows_import_precedence_and_inherits_method() {
    let module = style("<xsl:output method='text' media-type='application/imported'/><xsl:template match='/'>text</xsl:template>");
    let kernel = kernel(&[("urn:file:media-defaults", module)]);
    for (output, expected) in [
        ("", "application/imported"),
        (
            "<xsl:output media-type='application/n-triples'/>",
            "application/n-triples",
        ),
    ] {
        let main = style(&format!(
            "<xsl:import href='urn:file:media-defaults'/>{output}"
        ));
        let rep = issue(&kernel, &main).expect("transforms imported stylesheet");
        assert_eq!(rep.repr_type.media_type, expected);
        assert_eq!(rep.bytes, b"text");
    }
}
