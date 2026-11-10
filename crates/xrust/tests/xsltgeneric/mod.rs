//! Tests for XSLT defined generically

use pkg_version::{pkg_version_major, pkg_version_minor, pkg_version_patch};
use qualname::NamespaceMap;
use std::rc::Rc;
use url::Url;
use xrust::item::{Item, Node, Sequence, SequenceTrait};
use xrust::output::OutputDefinition;
use xrust::transform::context::StaticContextBuilder;
use xrust::value::Value;
use xrust::xdmerror::{Error, ErrorKind};
use xrust::xslt::from_document;

fn test_rig<N: Node, G, H, J>(
    src: impl AsRef<str>,
    style: impl AsRef<str>,
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(Sequence<N>, N), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    test_rig_with_params(
        src,
        style,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        None,
        vec![],
        vec![],
    )
}

fn test_rig_with_params<N: Node, G, H, J>(
    src: impl AsRef<str>,
    style: impl AsRef<str>,
    parse_from_str: G,
    _parse_from_str_with_ns: J,
    make_doc: H,
    base_url: Option<Url>,
    params: Vec<(String, String)>,
    externals: Vec<(String, String)>,
) -> Result<(Sequence<N>, N), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let srcdoc = parse_from_str(src.as_ref()).map_err(|e| {
        Error::new(
            e.kind,
            format!("error parsing source document: {}", e.message),
        )
    })?;
    let styledoc = parse_from_str(style.as_ref())
        .map_err(|e| Error::new(e.kind, format!("error parsing stylesheet: {}", e.message)))?;

    let mut stctxt = StaticContextBuilder::new()
        .message(|m| {
            eprintln!("stylesheet message: {}", m);
            Ok(())
        })
        .fetcher(|url| {
            externals.iter().fold(
                Err(Error::new(
                    ErrorKind::DynamicAbsent,
                    format!("resource not found: {}", url.to_string()),
                )),
                |acc, e| {
                    if url.to_string() == e.0 {
                        Ok(e.1.clone())
                    } else {
                        acc
                    }
                },
            )
        })
        .parser(|s| parse_from_str(s))
        .build();
    let mut ctxt = from_document(styledoc, None, |s| parse_from_str(s), |_| Ok(String::new()))?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    // Make sure the document lives until the end of the function's scope
    let rd = make_doc()?;
    ctxt.result_document(rd.clone());
    if base_url.is_some() {
        ctxt.set_baseurl(base_url.unwrap())
    }
    params.iter().for_each(|p| {
        ctxt.var_push(
            p.0.clone(),
            vec![Item::Value(Rc::new(Value::from(p.1.clone())))],
        )
    });

    ctxt.manage_whitespace(srcdoc)?;
    let result = ctxt.evaluate(&mut stctxt);

    result.map(|r| (r, rd))
}

fn test_msg_rig<N: Node, G, H, J>(
    src: impl AsRef<str>,
    style: impl AsRef<str>,
    parse_from_str: G,
    _parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(Sequence<N>, Vec<String>, N), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let srcdoc = parse_from_str(src.as_ref())?;
    let styledoc = parse_from_str(style.as_ref())?;
    let mut msgs: Vec<String> = vec![];
    let mut stctxt = StaticContextBuilder::new()
        .message(|m| {
            msgs.push(String::from(m));
            Ok(())
        })
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "not implemented")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "not implemented")))
        .build();
    let mut ctxt = from_document(styledoc, None, |s| parse_from_str(s), |_| Ok(String::new()))?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    let rd = make_doc()?;
    ctxt.result_document(rd.clone());
    let seq = ctxt.evaluate(&mut stctxt)?;
    Ok((seq, msgs, rd))
}

pub fn generic_literal_text<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'>Found the document</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "Found the document" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"Found the document\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn style_pos<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<!-- This is a stylesheet, despite having a comment in the prologue -->
        <xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'>Found the document</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "Found the document" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"Found the document\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_sys_prop<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:sequence select='system-property("xsl:version")'/>-<xsl:sequence select='system-property("xsl:product-version")'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string()
        == format!(
            "0.9-{}.{}.{}",
            pkg_version_major!(),
            pkg_version_minor!(),
            pkg_version_patch!()
        )
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"{}\"",
                result.0.to_string(),
                format!(
                    "0.9-{}.{}.{}",
                    pkg_version_major!(),
                    pkg_version_minor!(),
                    pkg_version_patch!()
                )
            ),
        ))
    }
}

pub fn generic_value_of_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>special &lt; less than</Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='child::*'><xsl:value-of select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "special < less than" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"special &lt; less than\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_value_of_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>special &lt; less than</Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='child::*'><xsl:value-of select='.' disable-output-escaping='yes'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "special < less than" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"special < less than\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_literal_element<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><answer>Made an element</answer></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "<answer>Made an element</answer>" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"<answer>Made an element</answer>\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_element<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:element name='answer{count(ancestor::*)}'>Made an element</xsl:element></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "<answer0>Made an element</answer0>" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"<answer0>Made an element</answer0>\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_apply_templates_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::*'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::text()'>found text</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "found textfound text" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"found textfound text\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_apply_templates_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>one<Level1/>two<Level1/>three<Level1/>four<Level1/></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates select='child::text()'/></xsl:template>
  <xsl:template match='child::Level1'>found Level1 element</xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "onetwothreefour" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"onetwothreefour\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_apply_templates_mode_bad_qname<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    // Specify a QName for a mode that has not been declared as an XML Namespace
    let result = test_rig(
        "<Test>one<Level1>a</Level1>two<Level1>b</Level1>three<Level1>c</Level1>four<Level1>d</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><HEAD><xsl:apply-templates select='child::Level1' mode='test:head'/></HEAD><BODY><xsl:apply-templates select='child::Level1' mode='body'/></BODY></xsl:template>
  <xsl:template match='child::Level1' mode='test:head'><h1><xsl:apply-templates/></h1></xsl:template>
  <xsl:template match='child::Level1' mode='test:body'><p><xsl:apply-templates/></p></xsl:template>
  <xsl:template match='child::Level1'>should not see this</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );
    if result.is_err() {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            "stylesheet succeeded, expected error",
        ))
    }
}

pub fn generic_apply_templates_mode<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>one<Level1>a</Level1>two<Level1>b</Level1>three<Level1>c</Level1>four<Level1>d</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><HEAD><xsl:apply-templates select='child::Level1' mode='head'/></HEAD><BODY><xsl:apply-templates select='child::Level1' mode='body'/></BODY></xsl:template>
  <xsl:template match='child::Level1' mode='head'><h1><xsl:apply-templates/></h1></xsl:template>
  <xsl:template match='child::Level1' mode='body'><p><xsl:apply-templates/></p></xsl:template>
  <xsl:template match='child::Level1'>should not see this</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml()
        == "<HEAD><h1>a</h1><h1>b</h1><h1>c</h1><h1>d</h1></HEAD><BODY><p>a</p><p>b</p><p>c</p><p>d</p></BODY>"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"<HEAD><h1>a</h1><h1>b</h1><h1>c</h1><h1>d</h1></HEAD><BODY><p>a</p><p>b</p><p>c</p><p>d</p></BODY>\"",
                result.0.to_xml()
            ),
        ))
    }
}

pub fn generic_apply_templates_sort<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>one<Level1>a</Level1>two<Level1>b</Level1>three<Level1>c</Level1>four<Level1>d</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates><xsl:sort select='.'/></xsl:apply-templates></xsl:template>
  <xsl:template match='child::Level1'><L><xsl:apply-templates/></L></xsl:template>
  <xsl:template match='child::Test/child::text()'><p><xsl:sequence select='.'/></p></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml()
        == "<L>a</L><L>b</L><L>c</L><L>d</L><p>four</p><p>one</p><p>three</p><p>two</p>"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"<L>a</L><L>b</L><L>c</L><L>d</L><p>four</p><p>one</p><p>three</p><p>two</p>\"",
                result.0.to_xml()
            ),
        ))
    }
}

pub fn generic_comment<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>one<Level1/>two<Level1/>three<Level1/>four<Level1/></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Level1'><xsl:comment> this is a level 1 element </xsl:comment></xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml()
        == "one<!-- this is a level 1 element -->two<!-- this is a level 1 element -->three<!-- this is a level 1 element -->four<!-- this is a level 1 element -->"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"one<!-- this is a level 1 element -->two<!-- this is a level 1 element -->three<!-- this is a level 1 element -->four<!-- this is a level 1 element -->\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_pi<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test>one<Level1/>two<Level1/>three<Level1/>four<Level1/></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Level1'><xsl:processing-instruction name='piL1'>this is a level 1 element</xsl:processing-instruction></xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml()
        == "one<?piL1 this is a level 1 element?>two<?piL1 this is a level 1 element?>three<?piL1 this is a level 1 element?>four<?piL1 this is a level 1 element?>"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"one<?piL1 this is a level 1 element?>two<?piL1 this is a level 1 element?>three<?piL1 this is a level 1 element?>four<?piL1 this is a level 1 element?>\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_current<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test ref='one'><second name='foo'>I am foo</second><second name='one'>I am one</second></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'>
    <xsl:sequence select='child::*/child::second[attribute::name eq current()/attribute::ref]'/>
  </xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    if result.0.to_xml() == "<second name='one'>I am one</second>" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"<second name='one'>I am one</second>\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_key_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><one>blue</one><two>yellow</two><three>green</three><four>blue</four></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:key name='mykey' match='child::*' use='child::text()'/>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'>#blue = <xsl:sequence select='count(key("mykey", "blue"))'/></xsl:template>
  <xsl:template match='child::Test/child::*'>shouldn't see this</xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "#blue = 2" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"#blue = 2\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn text_output_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><one>blue</one><two>yellow</two><three>green</three><four>blue</four></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:output method='text'/>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test/child::*'>
    <xsl:text>Colour: &quot;</xsl:text>
    <xsl:apply-templates/>
    <xsl:text>&quot;&#xa;</xsl:text>
  </xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string()
        == "Colour: \"blue\"\nColour: \"yellow\"\nColour: \"green\"\nColour: \"blue\"\n"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!("got result \"{}\"", result.0.to_string()),
        ))
    }
}

// Although we have the source and stylesheet in files,
// they are inlined here to avoid dependency on I/O libraries
pub fn generic_issue_58<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        r#"<Example>
    <Title>XSLT in Rust</Title>
    <Paragraph>A simple document.</Paragraph>
</Example>
"#,
        r#"<xsl:stylesheet
	version="1.0"
	xmlns:dat="http://www.stormware.cz/schema/version_2/data.xsd"
	xmlns:int="http://www.stormware.cz/schema/version_2/intDoc.xsd"
	xmlns:xsl="http://www.w3.org/1999/XSL/Transform">

	<xsl:output method="xml" encoding="utf-8" indent="yes"/>

    <xsl:template match="child::Example">
        <dat:dataPack>
            <xsl:apply-templates/>
        </dat:dataPack>
    </xsl:template>
    <xsl:template match="child::Title">
        <int:head>
            <xsl:apply-templates/>
        </int:head>
    </xsl:template>
    <xsl:template match="child::Paragraph">
        <int:body>
            <xsl:apply-templates/>
        </int:body>
    </xsl:template>
</xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml()
        == r#"<dat:dataPack xmlns:dat='http://www.stormware.cz/schema/version_2/data.xsd'>
    <int:head xmlns:int='http://www.stormware.cz/schema/version_2/intDoc.xsd'>XSLT in Rust</int:head>
    <int:body xmlns:int='http://www.stormware.cz/schema/version_2/intDoc.xsd'>A simple document.</int:body>
</dat:dataPack>"# {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!("not expected result"),
        ))
    }
}

pub fn generic_issue_95<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
          <xsl:template match="@*|node()">
            <xsl:copy>
              <xsl:apply-templates select="@*|node()"/>
            </xsl:copy>
          </xsl:template>
        </xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_xml() == "<Test><Level1>one</Level1><Level1>two</Level1></Test>" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"Found the document\"",
                result.0.to_xml()
            ),
        ))
    }
}

pub fn generic_message_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let (result, msgs, _rd) = test_msg_rig(
        "<Test>one<Level1/>two<Level1/>three<Level1/>four<Level1/></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Level1'><xsl:message>here is a level 1 element</xsl:message><L/></xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.to_xml() == "one<L/>two<L/>three<L/>four<L/>" {
        if msgs.len() == 4 {
            if msgs[0] == "here is a level 1 element" {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Unknown,
                    format!(
                        "got message \"{}\", expected \"here is a level 1 element\"",
                        msgs[0]
                    ),
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::Unknown,
                format!("got {} messages, expected 4", msgs.len()),
            ))
        }
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"one<L></L>two<L></L>three<L></L>four<L></L>\"",
                result.to_string()
            ),
        ))
    }
}

pub fn generic_message_term<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    match test_msg_rig(
        "<Test>one<Level1/>two<Level1/>three<Level1/>four<Level1/></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Level1'><xsl:message terminate='yes'>here is a level 1 element</xsl:message><L/></xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    ) {
        Err(e) => {
            if e.kind == ErrorKind::Terminated
                && e.message == "here is a level 1 element"
                && e.code.is_some_and(|f| f.local_name() == *"XTMM9000")
            {
                Ok(())
            } else {
                Err(Error::new(ErrorKind::Unknown, "incorrect error"))
            }
        }
        Ok(_) => Err(Error::new(
            ErrorKind::Unknown,
            "evaluation succeeded when it should have failed",
        )),
    }
}
pub fn generic_callable_named_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><one>blue</one><two>yellow</two><three>green</three><four>blue</four></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'>
    <xsl:call-template name='my_template'>
      <xsl:with-param name='my_param' select='count(child::*)'/>
    </xsl:call-template>
  </xsl:template>
  <xsl:template name='my_template'>
    <xsl:param name='my_param'>default value</xsl:param>
    <xsl:text>There are </xsl:text>
    <xsl:sequence select='$my_param'/>
    <xsl:text> child elements</xsl:text>
  </xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "There are 4 child elements" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"There are 4 child elements\"",
                result.0.to_string()
            ),
        ))
    }
}
pub fn callable_recurse_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><one>blue</one><two>yellow</two><three>green</three><four>blue</four></Test>",
        r###"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'>
    <result>
      <xsl:call-template name='my_template'>
        <xsl:with-param name='my_param' select='"black white red green blue"'/>
      </xsl:call-template>
    </result>
  </xsl:template>
  <xsl:template name='my_template'>
    <xsl:param name='my_param'>default value</xsl:param>
    <xsl:message>my_template(<xsl:value-of select="$my_param"/>)</xsl:message>
    <xsl:choose>
      <xsl:when test="$my_param eq ''">
        <xsl:message>stop recursion - no input</xsl:message>
      </xsl:when>
      <xsl:when test="not(contains($my_param, ' '))">
        <xsl:message>stop recursion - last input</xsl:message>

        <out>
          <xsl:value-of select="$my_param"/>
        </out>
      </xsl:when>
      <xsl:otherwise>
        <xsl:message>continue recursion - with new input "<xsl:value-of select="substring-after($my_param, ' ')"/>"</xsl:message>
        <out>
          <xsl:value-of select="substring-before($my_param, ' ')"/>
        </out>
        <xsl:call-template name='my_template'>
          <xsl:with-param name='my_param' select='substring-after($my_param, " ")'/>
        </xsl:call-template>
      </xsl:otherwise>
    </xsl:choose>
  </xsl:template>
</xsl:stylesheet>"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.0.to_xml(),
        r###"<result><out>black</out><out>white</out><out>red</out><out>green</out><out>blue</out></result>"###
    );
    Ok(())
}
pub fn generic_callable_posn_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><one>blue</one><two>yellow</two><three>green</three><four>blue</four></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform' xmlns:eg='http://example.org/'>
  <xsl:template match='/'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Test'>
    <xsl:sequence select='eg:my_func(count(child::*))'/>
  </xsl:template>
  <xsl:function name='eg:my_func'>
    <xsl:param name='my_param'/>
    <xsl:text>There are </xsl:text>
    <xsl:sequence select='$my_param'/>
    <xsl:text> child elements</xsl:text>
  </xsl:function>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    if result.0.to_string() == "There are 4 child elements" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"There are 4 child elements\"",
                result.0.to_string()
            ),
        ))
    }
}

pub fn generic_include<N: Node, G, H, J>(
    parse_from_str: G,
    _parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let srcdoc =
        parse_from_str("<Test>one<Level1/>two<Level2/>three<Level3/>four<Level4/></Test>")?;
    let styledoc = parse_from_str(
        "<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:include href='included.xsl'/>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::Level1'>found Level1 element</xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>",
    )?;
    let pwd = std::env::current_dir().expect("unable to get current directory");
    let pwds = pwd
        .into_os_string()
        .into_string()
        .expect("unable to convert pwd");
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_| Err(Error::new(ErrorKind::NotImplemented, "not implemented")))
        .parser(|_| Err(Error::new(ErrorKind::NotImplemented, "not implemented")))
        .build();
    let mut ctxt = from_document(
        styledoc,
        Some(
            Url::parse(format!("file://{}/tests/xsl/including.xsl", pwds.as_str()).as_str())
                .expect("unable to parse URL"),
        ),
        |s| parse_from_str(s),
        |_| Ok(String::new()),
    )?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    ctxt.result_document(make_doc()?);
    let result = ctxt.evaluate(&mut stctxt)?;
    if result.to_string()
        == "onefound Level1 elementtwofound Level2 elementthreefound Level3 elementfour"
    {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"onefound Level1 elementtwofound Level2 elementthreefound Level3 elementfour\"",
                result.to_string()
            ),
        ))
    }
}

pub fn generic_document_1<N: Node, G, H, J>(
    parse_from_str: G,
    _parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let srcdoc = parse_from_str("<Test><internal>on the inside</internal></Test>")?;
    let styledoc = parse_from_str(
        r##"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='child::Test'><xsl:apply-templates/>|<xsl:apply-templates select='document("urn::test.org/test")'/></xsl:template>
  <xsl:template match='child::internal'>found internal element</xsl:template>
  <xsl:template match='child::external'>found external element</xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"##,
    )?;
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_url| {
            Ok(String::from(
                "<Outside><external>from outside</external></Outside>",
            ))
        })
        .parser(|s| parse_from_str(s))
        .build();
    let mut ctxt = from_document(styledoc, None, |s| parse_from_str(s), |_| Ok(String::new()))?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    ctxt.result_document(make_doc()?);
    let result = ctxt.evaluate(&mut stctxt)?;
    if result.to_string() == "found internal element|found external element" {
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::Unknown,
            format!(
                "got result \"{}\", expected \"onefound Level1 elementtwofound Level2 elementthreefound Level3 elementfour\"",
                result.to_string()
            ),
        ))
    }
}

pub fn generic_number_1<N: Node, G, H, J>(
    parse_from_str: G,
    _parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let srcdoc = parse_from_str("<Test><t>one</t><t>two</t><t>three</t></Test>")?;
    let styledoc = parse_from_str(
        r##"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='child::Test'><xsl:apply-templates/></xsl:template>
  <xsl:template match='child::t'>t element <xsl:number/></xsl:template>
  <xsl:template match='child::text()'><xsl:sequence select='.'/></xsl:template>
</xsl:stylesheet>"##,
    )?;
    let mut stctxt = StaticContextBuilder::new()
        .message(|_| Ok(()))
        .fetcher(|_url| Ok(String::new()))
        .parser(|s| parse_from_str(s))
        .build();
    let mut ctxt = from_document(styledoc, None, |s| parse_from_str(s), |_| Ok(String::new()))?;
    ctxt.context(vec![Item::Node(srcdoc.clone())], 0);
    ctxt.result_document(make_doc()?);
    let result = ctxt.evaluate(&mut stctxt)?;
    assert_eq!(result.to_string(), "t element 1t element 2t element 3");
    Ok(())
}

pub fn attr_set_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:attribute-set name='foo'>
    <xsl:attribute name='bar'>from set foo</xsl:attribute>
  </xsl:attribute-set>
  <xsl:template match='child::Level1'><xsl:copy xsl:use-attribute-sets='foo'/></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.0.to_xml(),
        "<Level1 bar='from set foo'/><Level1 bar='from set foo'/>"
    );
    Ok(())
}

pub fn attr_set_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:attribute-set name='foo'>
    <xsl:attribute name='bar'>from set foo</xsl:attribute>
  </xsl:attribute-set>
  <xsl:template match='child::Level1'><MyElement xsl:use-attribute-sets='foo'><xsl:apply-templates/></MyElement></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.0.to_xml(),
        "<MyElement bar='from set foo'>one</MyElement><MyElement bar='from set foo'>two</MyElement>"
    );
    Ok(())
}

pub fn attr_set_3<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Test><Level1>one</Level1><Level1>two</Level1></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:attribute-set name='foo'>
    <xsl:attribute name='bar'>from set foo</xsl:attribute>
  </xsl:attribute-set>
  <xsl:template match='child::Level1'><xsl:element name='Element' xsl:use-attribute-sets='foo'><xsl:apply-templates/></xsl:element></xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.0.to_xml(),
        "<Element bar='from set foo'>one</Element><Element bar='from set foo'>two</Element>"
    );
    Ok(())
}

pub fn feg_starting_with_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let (result, _rd) = test_rig(
        "<Test><a>one</a><b>two</b><c>three</c><a>four</a><b>five</b><c>six</c><a>seven</a><b>eight</b><c>nine</c></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='Test'>
    <result>
      <xsl:for-each-group select='*' group-starting-with='a'>
        <group>
          <xsl:apply-templates select='current-group()'/>
        </group>
      </xsl:for-each-group>
    </result>
  </xsl:template>
  <xsl:template match='*'>
    <xsl:copy>
      <xsl:apply-templates/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.to_xml(),
        "<result><group><a>one</a><b>two</b><c>three</c></group><group><a>four</a><b>five</b><c>six</c></group><group><a>seven</a><b>eight</b><c>nine</c></group></result>"
    );
    Ok(())
}

pub fn feg_starting_with_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let (result, _rd) = test_rig(
        "<Test><a>one</a><b>two</b><c>three</c><a>four</a><b>five</b><c>six</c><a>seven</a><b>eight</b><c>nine</c></Test>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='Test'>
    <result>
      <xsl:for-each-group select='*' group-starting-with='a'>
        <group>
          <first>
            <xsl:apply-templates select='current-group()[position() eq 1]'/>
          </first>
          <rest>
            <xsl:apply-templates select='current-group()[position() ne 1]'/>
          </rest>
        </group>
      </xsl:for-each-group>
    </result>
  </xsl:template>
  <xsl:template match='*'>
    <xsl:copy>
      <xsl:apply-templates/>
    </xsl:copy>
  </xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.to_xml(),
        "<result><group><first><a>one</a></first><rest><b>two</b><c>three</c></rest></group><group><first><a>four</a></first><rest><b>five</b><c>six</c></rest></group><group><first><a>seven</a></first><rest><b>eight</b><c>nine</c></rest></group></result>"
    );
    Ok(())
}

pub fn issue_96_abs<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Example><Level1>one</Level1><Level1>two</Level1></Example>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='/Example'>found an Example element</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(result.0.to_xml(), "found an Example element");
    Ok(())
}

pub fn issue_96_rel<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Example><Level1>one</Level1><Level1>two</Level1></Example>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='Example'>found an Example element</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(result.0.to_xml(), "found an Example element");
    Ok(())
}

pub fn issue_96_mixed<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Example><Level1>one</Level1><Level1>two</Level1></Example>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
  <xsl:template match='child::Example/Level1'>found a Level1 element</xsl:template>
</xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.0.to_xml(),
        "found a Level1 elementfound a Level1 element"
    );
    Ok(())
}

pub fn issue_126<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<catalog>
          <cd>
            <title>Empire Burlesque</title>
            <artist>Bob Dylan</artist>
            <country>USA</country>
            <company>Columbia</company>
            <price>10.90</price>
            <year>1985</year>
          </cd>
          <cd>
            <title>Hide your heart</title>
            <artist>Bonnie Tyler</artist>
            <country>UK</country>
            <company>CBS Records</company>
            <price>9.90</price>
            <year>1988</year>
          </cd>
          <cd>
            <title>Greatest Hits</title>
            <artist>Dolly Parton</artist>
            <country>USA</country>
            <company>RCA</company>
            <price>9.90</price>
            <year>1982</year>
          </cd>
          <cd>
            <title>Still got the blues</title>
            <artist>Gary Moore</artist>
            <country>UK</country>
            <company>Virgin records</company>
            <price>10.20</price>
            <year>1990</year>
          </cd>
          <cd>
            <title>Eros</title>
            <artist>Eros Ramazzotti</artist>
            <country>EU</country>
            <company>BMG</company>
            <price>9.90</price>
            <year>1997</year>
          </cd>
          <cd>
            <title>One night only</title>
            <artist>Bee Gees</artist>
            <country>UK</country>
            <company>Polydor</company>
            <price>10.90</price>
            <year>1998</year>
          </cd>
          <cd>
            <title>Sylvias Mother</title>
            <artist>Dr.Hook</artist>
            <country>UK</country>
            <company>CBS</company>
            <price>8.10</price>
            <year>1973</year>
          </cd>
          <cd>
            <title>Maggie May</title>
            <artist>Rod Stewart</artist>
            <country>UK</country>
            <company>Pickwick</company>
            <price>8.50</price>
            <year>1990</year>
          </cd>
          <cd>
            <title>Romanza</title>
            <artist>Andrea Bocelli</artist>
            <country>EU</country>
            <company>Polydor</company>
            <price>10.80</price>
            <year>1996</year>
          </cd>
          <cd>
            <title>When a man loves a woman</title>
            <artist>Percy Sledge</artist>
            <country>USA</country>
            <company>Atlantic</company>
            <price>8.70</price>
            <year>1987</year>
          </cd>
          <cd>
            <title>Black angel</title>
            <artist>Savage Rose</artist>
            <country>EU</country>
            <company>Mega</company>
            <price>10.90</price>
            <year>1995</year>
          </cd>
          <cd>
            <title>1999 Grammy Nominees</title>
            <artist>Many</artist>
            <country>USA</country>
            <company>Grammy</company>
            <price>10.20</price>
            <year>1999</year>
          </cd>
          <cd>
            <title>For the good times</title>
            <artist>Kenny Rogers</artist>
            <country>UK</country>
            <company>Mucik Master</company>
            <price>8.70</price>
            <year>1995</year>
          </cd>
          <cd>
            <title>Big Willie style</title>
            <artist>Will Smith</artist>
            <country>USA</country>
            <company>Columbia</company>
            <price>9.90</price>
            <year>1997</year>
          </cd>
          <cd>
            <title>Tupelo Honey</title>
            <artist>Van Morrison</artist>
            <country>UK</country>
            <company>Polydor</company>
            <price>8.20</price>
            <year>1971</year>
          </cd>
          <cd>
            <title>Soulsville</title>
            <artist>Jorn Hoel</artist>
            <country>Norway</country>
            <company>WEA</company>
            <price>7.90</price>
            <year>1996</year>
          </cd>
          <cd>
            <title>The very best of</title>
            <artist>Cat Stevens</artist>
            <country>UK</country>
            <company>Island</company>
            <price>8.90</price>
            <year>1990</year>
          </cd>
          <cd>
            <title>Stop</title>
            <artist>Sam Brown</artist>
            <country>UK</country>
            <company>A and M</company>
            <price>8.90</price>
            <year>1988</year>
          </cd>
          <cd>
            <title>Bridge of Spies</title>
            <artist>T`Pau</artist>
            <country>UK</country>
            <company>Siren</company>
            <price>7.90</price>
            <year>1987</year>
          </cd>
          <cd>
            <title>Private Dancer</title>
            <artist>Tina Turner</artist>
            <country>UK</country>
            <company>Capitol</company>
            <price>8.90</price>
            <year>1983</year>
          </cd>
          <cd>
            <title>Midt om natten</title>
            <artist>Kim Larsen</artist>
            <country>EU</country>
            <company>Medley</company>
            <price>7.80</price>
            <year>1983</year>
          </cd>
          <cd>
            <title>Pavarotti Gala Concert</title>
            <artist>Luciano Pavarotti</artist>
            <country>UK</country>
            <company>DECCA</company>
            <price>9.90</price>
            <year>1991</year>
          </cd>
          <cd>
            <title>The dock of the bay</title>
            <artist>Otis Redding</artist>
            <country>USA</country>
            <COMPANY>Stax Records</COMPANY>
            <PRICE>7.90</PRICE>
            <YEAR>1968</YEAR>
          </cd>
          <cd>
            <title>Picture book</title>
            <artist>Simply Red</artist>
            <country>EU</country>
            <company>Elektra</company>
            <price>7.20</price>
            <year>1985</year>
          </cd>
          <cd>
            <title>Red</title>
            <artist>The Communards</artist>
            <country>UK</country>
            <company>London</company>
            <price>7.80</price>
            <year>1987</year>
          </cd>
          <cd>
            <title>Unchain my heart</title>
            <artist>Joe Cocker</artist>
            <country>USA</country>
            <company>EMI</company>
            <price>8.20</price>
            <year>1987</year>
          </cd>
        </catalog>",
        r###"<xsl:stylesheet version="1.0"
        xmlns:xsl="http://www.w3.org/1999/XSL/Transform">

        <xsl:template match="/">
          <html>
          <body>
          <h2>My CD Collection</h2>
          <table border="1">
            <ts bgcolor="#9acd32">
              <th>Title</th>
              <th>Artist</th>
            </ts>
            <xsl:for-each select="catalog/cd">
            <tr>
              <td><xsl:value-of select="title"/></td>
              <td><xsl:value-of select="artist"/></td>
            </tr>
            </xsl:for-each>
          </table>
          </body>
          </html>
        </xsl:template>

        </xsl:stylesheet>"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;
    assert_eq!(
        result.0.to_xml(),
        r###"<html><body><h2>My CD Collection</h2><table border='1'><ts bgcolor='#9acd32'><th>Title</th><th>Artist</th></ts><tr><td>Empire Burlesque</td><td>Bob Dylan</td></tr><tr><td>Hide your heart</td><td>Bonnie Tyler</td></tr><tr><td>Greatest Hits</td><td>Dolly Parton</td></tr><tr><td>Still got the blues</td><td>Gary Moore</td></tr><tr><td>Eros</td><td>Eros Ramazzotti</td></tr><tr><td>One night only</td><td>Bee Gees</td></tr><tr><td>Sylvias Mother</td><td>Dr.Hook</td></tr><tr><td>Maggie May</td><td>Rod Stewart</td></tr><tr><td>Romanza</td><td>Andrea Bocelli</td></tr><tr><td>When a man loves a woman</td><td>Percy Sledge</td></tr><tr><td>Black angel</td><td>Savage Rose</td></tr><tr><td>1999 Grammy Nominees</td><td>Many</td></tr><tr><td>For the good times</td><td>Kenny Rogers</td></tr><tr><td>Big Willie style</td><td>Will Smith</td></tr><tr><td>Tupelo Honey</td><td>Van Morrison</td></tr><tr><td>Soulsville</td><td>Jorn Hoel</td></tr><tr><td>The very best of</td><td>Cat Stevens</td></tr><tr><td>Stop</td><td>Sam Brown</td></tr><tr><td>Bridge of Spies</td><td>T`Pau</td></tr><tr><td>Private Dancer</td><td>Tina Turner</td></tr><tr><td>Midt om natten</td><td>Kim Larsen</td></tr><tr><td>Pavarotti Gala Concert</td><td>Luciano Pavarotti</td></tr><tr><td>The dock of the bay</td><td>Otis Redding</td></tr><tr><td>Picture book</td><td>Simply Red</td></tr><tr><td>Red</td><td>The Communards</td></tr><tr><td>Unchain my heart</td><td>Joe Cocker</td></tr></table></body></html>"###
    );
    Ok(())
}

pub fn issue_137_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<dummy/>",
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
        <xsl:variable name="title">A really exciting document</xsl:variable>
        <xsl:variable name="backcolor" select="'#FFFFCC'"/>
        <xsl:template match="/*">
            <HTML><TITLE><xsl:value-of select="$title"/></TITLE>
            <BODY BGCOLOR='{$backcolor}'>
                <!-- ... -->
            </BODY></HTML>
        </xsl:template>
        </xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.0.to_xml(),
        "<HTML><TITLE>A really exciting document</TITLE><BODY BGCOLOR='#FFFFCC'/></HTML>"
    );
    Ok(())
}

pub fn issue_137_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<dummy/>",
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
        <xsl:variable name="title">A really exciting document</xsl:variable>
        <xsl:variable name="body-title" select="concat('Title: ', $title)"/>
        <xsl:variable name="backcolor" select="'#FFFFCC'"/>
        <xsl:template match="/*">
            <HTML><TITLE><xsl:value-of select="$title"/></TITLE>
            <BODY BGCOLOR='{$backcolor}'>
                <H1><xsl:value-of select="$body-title"/></H1>
            </BODY></HTML>
        </xsl:template>
        </xsl:stylesheet>"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.0.to_xml(),
        "<HTML><TITLE>A really exciting document</TITLE><BODY BGCOLOR='#FFFFCC'><H1>Title: A really exciting document</H1></BODY></HTML>"
    );
    Ok(())
}

pub fn dbk_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let (result, _rd) = test_rig(
        "<db:article xmlns:db='http://docbook.org/ns/docbook'>
        <db:sect1 xmlns:db='http://docbook.org/ns/docbook'>
          <db:title xmlns:db='http://docbook.org/ns/docbook'>Level 1 Heading</db:title>
          <db:para xmlns:db='http://docbook.org/ns/docbook'>First paragraph</db:para>
          <db:sect2 xmlns:db='http://docbook.org/ns/docbook'>
            <db:title xmlns:db='http://docbook.org/ns/docbook'>Level 2 Heading</db:title>
            <db:para xmlns:db='http://docbook.org/ns/docbook'>Second paragraph</db:para>
          </db:sect2>
          <db:sect2 xmlns:db='http://docbook.org/ns/docbook'>
            <db:title xmlns:db='http://docbook.org/ns/docbook'>Second Level 2 Heading</db:title>
            <db:para xmlns:db='http://docbook.org/ns/docbook'>Third paragraph</db:para>
          </db:sect2>
        </db:sect1>
        <db:sect1 xmlns:db='http://docbook.org/ns/docbook'>
          <db:title xmlns:db='http://docbook.org/ns/docbook'>Second Level 1 Heading</db:title>
          <db:para xmlns:db='http://docbook.org/ns/docbook'>Fourth paragraph</db:para>
          <db:sect2 xmlns:db='http://docbook.org/ns/docbook'>
            <db:title xmlns:db='http://docbook.org/ns/docbook'>Third Level 2 Heading</db:title>
            <db:para xmlns:db='http://docbook.org/ns/docbook'>Fifth paragraph</db:para>
          </db:sect2>
        </db:sect1></db:article>
",
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
          xmlns:db="http://docbook.org/ns/docbook"
          version="3.0">

          <xsl:output indent="yes"/>
          <xsl:strip-space elements="*"/>

          <xsl:template match="db:article">
            <HTML>
            <BODY>
            <xsl:apply-templates/>
            </BODY>
            </HTML>
          </xsl:template>
          <xsl:template match="*">
            <DIV>matched element <I><xsl:sequence select="name()"/></I></DIV>
          </xsl:template>
          <xsl:template match="db:sect1">
            <DIV CLASS="sect1"><xsl:apply-templates/></DIV>
          </xsl:template>
          <xsl:template match="db:sect2">
            <DIV CLASS="sect2"><xsl:apply-templates/></DIV>
          </xsl:template>
          <xsl:template match="db:title">
            <H1><xsl:apply-templates/></H1>
          </xsl:template>
          <xsl:template match="db:para">
            <P><xsl:apply-templates/></P>
          </xsl:template>
          <xsl:template match="db:emphasis">
            <I><xsl:apply-templates/></I>
          </xsl:template>
        </xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    // The output definition should be taken from the stylesheet,
    // but we'll fake it for now

    let mut od = OutputDefinition::new();
    od.set_indent(true);

    assert_eq!(
        result.to_xml_with_options(&od),
        "<HTML>
<BODY>
  <DIV CLASS='sect1'>
    <H1>Level 1 Heading</H1>
    <P>First paragraph</P>
    <DIV CLASS='sect2'>
      <H1>Level 2 Heading</H1>
      <P>Second paragraph</P>
    </DIV>
    <DIV CLASS='sect2'>
      <H1>Second Level 2 Heading</H1>
      <P>Third paragraph</P>
    </DIV>
  </DIV>
  <DIV CLASS='sect1'>
    <H1>Second Level 1 Heading</H1>
    <P>Fourth paragraph</P>
    <DIV CLASS='sect2'>
      <H1>Third Level 2 Heading</H1>
      <P>Fifth paragraph</P>
    </DIV>
  </DIV>
</BODY></HTML>"
    );
    Ok(())
}

pub fn md_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let (result, _rd) = test_rig(
        "<article>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </article>
",
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
          xmlns:db="http://docbook.org/ns/docbook"
          version="3.0">

          <xsl:output indent="yes"/>
          <xsl:strip-space elements="*"/>

          <xsl:template match="article">
            <db:article>
              <xsl:for-each-group select="*" group-starting-with="heading1">
                <xsl:choose>
                  <xsl:when test="current-group()[position() eq 1]/self::heading1">
                    <db:sect1>
                      <xsl:apply-templates select="current-group()[position() eq 1]"/>
                      <xsl:for-each-group select="current-group()[position() ne 1]"
                        group-starting-with="heading2">
                        <xsl:choose>
                          <xsl:when test="current-group()[position() eq 1]/self::heading2">
                            <db:sect2>
                              <xsl:apply-templates select="current-group()"/>
                            </db:sect2>
                          </xsl:when>
                          <xsl:otherwise>
                            <xsl:apply-templates select="current-group()"/>
                          </xsl:otherwise>
                        </xsl:choose>
                      </xsl:for-each-group>
                    </db:sect1>
                  </xsl:when>
                  <xsl:otherwise>
                    <xsl:apply-templates select="current-group()"/>
                  </xsl:otherwise>
                </xsl:choose>
              </xsl:for-each-group>
            </db:article>
          </xsl:template>
          <xsl:template match="heading1|heading2">
            <db:title>
              <xsl:apply-templates/>
            </db:title>
          </xsl:template>
          <xsl:template match="para">
            <db:para>
              <xsl:apply-templates/>
            </db:para>
          </xsl:template>
          <xsl:template match="emph">
            <db:emphasis>
              <xsl:apply-templates select="@*|node()"/>
            </db:emphasis>
          </xsl:template>
          <xsl:template match="attribute::role|attribute::id">
            <xsl:copy/>
          </xsl:template>
          <!--xsl:template match="emph[@role eq 'strong']">
            <db:emphasis role="strong">
              <xsl:apply-templates select="@*|node()"/>
            </db:emphasis>
          </xsl:template>
          <xsl:template match="emph[@role eq 'underline']">
            <db:emphasis role="underline">
              <xsl:apply-templates select="@*|node()"/>
            </db:emphasis>
          </xsl:template-->
        </xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.to_xml(),
        "<db:article xmlns:db='http://docbook.org/ns/docbook'><db:sect1><db:title>Level 1 Heading</db:title><db:para>First paragraph with <db:emphasis>emphasised</db:emphasis> text, <db:emphasis role='strong'>bold</db:emphasis> text, and <db:emphasis role='underline'>underlined</db:emphasis> text</db:para></db:sect1></db:article>"
    );
    Ok(())
}

pub fn ghissue_147<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<document>
        <listitem><paragraph>first</paragraph></listitem>
        <listitem><paragraph>second</paragraph></listitem>
        </document>
",
        r#"<?xml version="1.0"?>
        <xsl:stylesheet
          xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
          version="1.0"
          xmlns:miramo="http://ExternalFunction.miramo.com"
          exclude-result-prefixes="miramo"
          xmlns:mmc="http://www.miramo.com/mmc"
          xmlns:xlink="http://xlink"
          >
          <xsl:output encoding="utf-8" indent="yes" />

          <!-- - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  -->
          <xsl:template match="document">
          <testXML>
            	<xsl:apply-templates/>
          </testXML>
          </xsl:template>


          <!-- - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  -->
          <!-- paragraph element maps to <P> whose name is defined from context -->
          <xsl:template match="paragraph">
              <P>
                <!-- Restart label numbering on first listitem -->
                <xsl:if test="local-name(..) = 'listitem' and not(../preceding-sibling::listitem)">
                  <xsl:attribute name="numberValue">1</xsl:attribute>
                </xsl:if>
                <xsl:apply-templates/>
              </P>
          </xsl:template>

        </xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.1.to_xml(),
        /* Original result document from bug report
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         *        r##"<?xml version="1.0" encoding="utf-8"?>
<testXML xmlns:mmc="http://www.miramo.com/mmc" xmlns:xlink="http://xlink">
<P numberValue="1">first</P>
<P>second</P>
</testXML>"##*/
        r##"<testXML>
        <P numberValue='1'>first</P>
        <P>second</P>
        </testXML>"##
    );
    Ok(())
}

pub fn gl_issue_147_a<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<Example><Title>XSLT in Rust<InnerTitle>is working ok!</InnerTitle></Title></Example>",
        r#"<xsl:stylesheet xmlns:xsl='http://www.w3.org/1999/XSL/Transform'>
            <xsl:template match='Example'>
                <example><xsl:apply-templates/></example>
            </xsl:template>
            <xsl:template match='Title'>
                <title><xsl:apply-templates select='text()'/><xsl:apply-templates select='InnerTitle'/></title>
            </xsl:template>
            <xsl:template match='InnerTitle'>
                <inner><xsl:apply-templates /></inner>
            </xsl:template>
        </xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.1.to_xml(),
        r#"<example><title>XSLT in Rust<inner>is working ok!</inner></title></example>"#
    );
    Ok(())
}

pub fn gl_issue_147_b<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        r#"<test:Example xmlns:test="http://test.org"><test:Title>XSLT in Rust<test:InnerTitle>is working ok!</test:InnerTitle></test:Title></test:Example>"#,
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                        xmlns:test="http://test.org">
          <xsl:template match="test:Example">
            <example><xsl:apply-templates/></example>
          </xsl:template>

          <xsl:template match="test:Title">
            <title>
              <xsl:apply-templates select="text()"/>
              <xsl:apply-templates select="test:InnerTitle"/>
            </title>
          </xsl:template>

          <xsl:template match="test:InnerTitle">
            <inner><xsl:apply-templates/></inner>
          </xsl:template>
        </xsl:stylesheet>
"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    )?;

    assert_eq!(
        result.1.to_xml(),
        r#"<example><title>XSLT in Rust<inner>is working ok!</inner></title></example>"#
    );
    Ok(())
}

pub fn conform_1<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<article>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </article>
",
        r#"<?xml version="1.0" encoding="utf-8"?>
        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		xmlns="http://www.w3.org/1999/xhtml"
		xmlns:h="http://www.w3.org/1999/xhtml"
		xmlns:f="http://docbook.org/xslt/ns/extension"
		xmlns:m="http://docbook.org/xslt/ns/mode"
		xmlns:fn="http://www.w3.org/2003/11/xpath-functions"
		xmlns:db="http://docbook.org/docbook-ng"
		exclude-result-prefixes="h f m fn db"
                        version="2.0">

        <xsl:variable name="dummy">
          <db:book>
              <db:info>
	<db:title>Book Title</db:title>
              </db:info>
            <db:chapter>
              <db:info>
	<db:title>ChapterTitle</db:title>
              </db:info>
              <db:para/>
            </db:chapter>
          </db:book>
        </xsl:variable>

        <xsl:template match="/">
          <xsl:apply-templates select="$dummy/db:book/db:chapter/db:info/db:title"
		       mode="m:titlepage-mode"/>
        </xsl:template>

        <xsl:template match="db:chapter/db:info/db:title
		     |db:appendix/db:info/db:title
		     |db:preface/db:info/db:title
		     |db:bibliography/db:info/db:title"
	      mode="m:titlepage-mode"
	      priority="100">

          <h2>
            <xsl:next-match/>
          </h2>
        </xsl:template>

        <xsl:template match="db:title" mode="m:titlepage-mode">
          <xsl:apply-templates/>
        </xsl:template>

        <xsl:template match="*" mode="m:titlepage-mode">
          <xsl:apply-templates select="."/>
        </xsl:template>

        </xsl:stylesheet>

"#,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert_eq!(result.is_err(), true);
    Ok(())
}

pub fn conform_2<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<article>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </article>
",
        r###"<?xml version="1.0" encoding="utf-8"?>
        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		xmlns="http://www.w3.org/1999/xhtml"
		xmlns:h="http://www.w3.org/1999/xhtml"
		xmlns:f="http://docbook.org/xslt/ns/extension"
		xmlns:m="http://docbook.org/xslt/ns/mode"
		xmlns:fn="http://www.w3.org/2003/11/xpath-functions"
		xmlns:db="http://docbook.org/docbook-ng"
		exclude-result-prefixes="h f m fn db"
                        version="2.0">

        <xsl:variable name="dummy">
          <db:book>
              <db:info>
	<db:title>Book Title</db:title>
              </db:info>
            <db:chapter>
              <db:info>
	<db:title>ChapterTitle</db:title>
              </db:info>
              <db:para/>
            </db:chapter>
          </db:book>
        </xsl:variable>

        <xsl:template match="/">
          <xsl:apply-templates select="$dummy/db:book/db:chapter/db:info/db:title"
		       mode="m:titlepage-mode"/>
        </xsl:template>

        <xsl:template match="db:chapter/db:info/db:title
		     |db:appendix/db:info/db:title
		     |db:preface/db:info/db:title
		     |db:bibliography/db:info/db:title"
	      mode="m:titlepage-mode"
	      priority="100">

          <h2>
            <xsl:next-match/>
          </h2>
        </xsl:template>

        <xsl:template match="db:title" mode="#all">
          <xsl:apply-templates/>
        </xsl:template>

        <xsl:template match="*" mode="m:titlepage-mode">
          <xsl:apply-templates select="."/>
        </xsl:template>

        </xsl:stylesheet>

"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_err());
    Ok(())
}

pub fn conform_3<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<article>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </article>
",
        r###"<?xml version="1.0" encoding="UTF-8"?>
        <t:transform xmlns:t="http://www.w3.org/1999/XSL/Transform" version="2.0">
        <!-- Purpose: Test of select in xsl:value-of with current function -->

           <t:variable name="var">
		    <doc xmlns:xsl="http://www.w3.org/1999/XSL/Transform">6<num1>1<num2>2<num3>3</num3>
                    </num2>
                 </num1>
                 <num4>4<num6>at3</num6>
                 </num4>
                 <num5>5</num5>
              </doc>
	  </t:variable>

           <t:template match="doc">
		    <out>
                 <t1>
                    <t:value-of select="name($var//*[string() = current()/@*])" separator="|"/>
                 </t1>
              </out>
	  </t:template>

           <t:template match="text()"/>
        </t:transform>

"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_ok());
    Ok(())
}

pub fn conform_4<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<article>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </article>
",
        r###"<?xml version="1.0" encoding="UTF-8"?>
        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0">

          <!-- Purpose: When no stylesheets are imported, an xsl:apply-imports should
               select the built-in templates. -->

        <xsl:template match="/">
          <result>
            Before apply-imports
              <xsl:apply-imports/>
            After apply-imports
          </result>
        </xsl:template>

        </xsl:stylesheet>

"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_ok());
    Ok(())
}

pub fn conform_5<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<doc>
          <c x='attribute'>test</c>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </doc>
",
        r###"<?xml version="1.0"?>
        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0">

        <?spec xpath#id-path-expressions?>
          <!-- Purpose: Tests following axis starting from an attribute. -->
          <!-- Author: Scott Boag -->

        <xsl:template match="/">
          <out>
            <xsl:for-each select="//c/@x">
              <xsl:apply-templates select="following::*"/>
            </xsl:for-each>
          </out>
        </xsl:template>

        <xsl:template match="*">
          <xsl:text> </xsl:text><xsl:value-of select="name()"/>
        </xsl:template>

        </xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_ok());
    Ok(())
}

pub fn conform_6<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<doc>
          <c x='attribute'>test</c>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </doc>
",
        r###"<?xml version="1.0"?>

        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0"
        xmlns:xs="http://www.w3.org/2001/XMLSchema" exclude-result-prefixes="xs">

        <?spec xslt#format-date?>
          <!-- PURPOSE: XSLT 2.0: test format-date: numeric formats using Thai digits -->

          <xsl:output encoding="iso-8859-1"/>

          <xsl:param name="d" as="xs:date" select="xs:date('2003-09-07')"/>

          <xsl:template name="main">
        <out>;
         <tr><code>2003-09-7</code><code x='{format-date($d,"[Y&#xe50;&#xe50;&#xe50;&#xe51;]-[M&#xe50;&#xe51;]-[D&#xe51;]")}'/></tr>;
         <tr><code>9-7-2003</code><code x='{format-date($d,"[M&#xe51;]-[D&#xe51;]-[Y&#xe50;&#xe50;&#xe50;&#xe51;]")}'/></tr>;
         <tr><code>(03-09-07)</code><code x='{format-date($d,"([Y&#xe50;&#xe51;]-[M&#xe50;&#xe51;]-[D&#xe50;&#xe51;])")}'/></tr>;
        </out>
          </xsl:template>

        </xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_ok());
    Ok(())
}

pub fn conform_7<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<doc><bind>HTTP</bind><bind>SOAP</bind></doc>",
        r###"<?xml version="1.0"?>
        <xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform" version="2.0"
          xmlns:x="http://namespaces.ogbuji.net/articles" exclude-result-prefixes="x">

          <!-- Purpose: Test combination of key() and document() reading from stylesheet. -->
          <!-- Elaboration: "Look-up table 1.6 is worth a close look because it uses an advanced XSLT
            technique. It builds up the lookup-table right in the stylesheet, using a distinct namespace.
            You can see the x:ns-to-binding elements right below the key. If you are familiar with keys,
            you are aware that they define indices that will be built on the nodes in the original source
            document that match the pattern in the match attribute. What is not as well known is that
            every time an additional source document is loaded with the XSLT document() function, all keys
            are applied to it as well. The xsl:variable...uses a special form of document() call to load
            the stylesheet itself as an additional source document. Thus the nodes in the stylesheet that
            match the ns-to-binding are indexed. This is a very useful technique for setting up a look-up
            table without having to hack at the source document or depend on an additional file." -->


          <!-- Lookup table 1.6: WSDL binding types -->
          <xsl:key name="ns-to-binding" match="x:ns-to-binding" use="@binding"/>
          <x:ns-to-binding uri="http://schemas.xmlsoap.org/wsdl/soap/" binding="SOAP"/>
          <x:ns-to-binding uri="http://schemas.xmlsoap.org/wsdl/mime/" binding="MIME"/>
          <x:ns-to-binding uri="http://schemas.xmlsoap.org/wsdl/http/" binding="HTTP"/>

          <xsl:template match="doc">
            <out>
              <xsl:apply-templates/>
            </out>
          </xsl:template>

          <xsl:template match="bind">
            <bound>
              <xsl:variable name="lookup" select="."/>
              <xsl:value-of select="$lookup"/>
              <xsl:text>- </xsl:text>
              <xsl:for-each select="document('')">
                <!-- Switch context so key reads from stylesheet -->
                <xsl:value-of select="key('ns-to-binding',$lookup)/@uri"/>
              </xsl:for-each>
            </bound>
          </xsl:template>

          <xsl:template match="text()"/>

        </xsl:stylesheet>

"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );

    assert!(result.is_err());
    Ok(())
}

pub fn conform_8<N: Node, G, H, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: H,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    H: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig(
        "<doc>
          <c x='attribute'>test</c>
          <heading1>Level 1 Heading</heading1>
          <para>First paragraph with <emph>emphasised</emph> text, <emph role='strong'>bold</emph> text, and <emph role='underline'>underlined</emph> text</para>
        </doc>
",
        r###"<?xml version='1.0' encoding='UTF-8' ?>
        <xsl:stylesheet version="3.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
            xmlns:f="http://local-functions/"
            xmlns:xs="http://www.w3.org/2001/XMLSchema"
            exclude-result-prefixes="f xs">

            <!-- test combinations of copy-namespaces and inherit-namespaces -->

            <xsl:param name="INHERIT" static="yes" select="true()"/>
            <xsl:param name="COPY" static="yes" select="true()"/>

            <xsl:variable name="outer">
                <a xmlns="uri:a">
                    <b:b xmlns:b="uri:b">
                        <c xmlns="uri:c" xmlns:d="uri:d">
                            <graft xmlns=""/>
                        </c>
                    </b:b>
                </a>
            </xsl:variable>

            <xsl:variable name="inner">
                <p xmlns="uri:p">
                    <q xmlns="uri:q">
                        <r xmlns="uri:r" xmlns:s="uri:s"/>
                    </q>
                </p>
            </xsl:variable>

            <xsl:mode on-no-match="shallow-copy"/>

            <xsl:template match="*">
                <xsl:copy _inherit-namespaces="{$INHERIT}">
                    <xsl:apply-templates/>
                </xsl:copy>
            </xsl:template>

            <xsl:template match="graft">
                <xsl:copy-of select="$inner" _copy-namespaces="{$COPY}"/>
            </xsl:template>

            <xsl:output method="xml" encoding="utf-8"/>
            <xsl:template name="main">
                <xsl:variable name="capture">
                    <xsl:apply-templates select="$outer"/>
                </xsl:variable>
                <out>
                    <xsl:for-each select="$capture//*">
                        <element name="{local-name()}" uri="{namespace-uri()}" in-scope="{f:in-scope-namespaces(.)}"/>
                    </xsl:for-each>
                </out>
            </xsl:template>

            <xsl:function name="f:in-scope-namespaces" as="xs:string">
                <xsl:param name="e" as="element(*)"/>
                <xsl:value-of>
                    <xsl:for-each select="in-scope-prefixes($e)[. != 'xml']">
                        <xsl:sort select="."/>
                        <xsl:value-of select="concat('|', ., '=', namespace-uri-for-prefix(., $e))"/>
                    </xsl:for-each>
                </xsl:value-of>
            </xsl:function>

        </xsl:stylesheet>

"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
    );
    assert!(result.is_ok());
    Ok(())
}

pub fn namespace_1<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{B4B4B9CF-A4A9-2146-844A-107C12CFFDC9}">
                   <sheetData>
                      <row r="1"
                           spans="1:15"
                           customFormat="1"
                           ht="39.950000000000003"
                           customHeight="1">
                         <c r="A1" s="23" t="s">
                            <v>25</v>
                         </c>
                         <c r="B1" s="112" t="s">
                            <v>95</v>
                         </c>
                         <c r="C1" s="112"/>
                         <c r="D1" s="112"/>
                         <c r="E1" s="112"/>
                         <c r="F1" s="112"/>
                         <c r="G1" s="112"/>
                         <c r="H1" s="112"/>
                         <c r="I1" s="112"/>
                         <c r="J1" s="112"/>
                         <c r="K1" s="112"/>
                         <c r="L1" s="112"/>
                         <c r="M1" s="23" t="s">
                            <v>2</v>
                         </c>
                      </row>
                      <row r="2" spans="1:15" ht="3.95" customHeight="1">
                         <c r="A2" s="25"/>
                         <c r="B2" s="25"/>
                         <c r="C2" s="25"/>
                         <c r="D2" s="25"/>
                         <c r="E2" s="25"/>
                         <c r="F2" s="25"/>
                         <c r="G2" s="25"/>
                         <c r="H2" s="25"/>
                         <c r="I2" s="25"/>
                         <c r="J2" s="25"/>
                         <c r="K2" s="25"/>
                         <c r="L2" s="25"/>
                         <c r="M2" s="25"/>
                         <c r="N2"/>
                         <c r="O2" s="5"/>
                      </row>
                      </sheetData>
                      <mergeCells count="7">
                         <mergeCell ref="L5:M5"/>
                         <mergeCell ref="B1:L1"/>
                         <mergeCell ref="A3:M3"/>
                         <mergeCell ref="A4:M4"/>
                         <mergeCell ref="B5:D5"/>
                         <mergeCell ref="E5:F5"/>
                         <mergeCell ref="G5:K5"/>
                      </mergeCells>
                      <pageMargins left="0.7"
                                   right="0.7"
                                   top="0.75"
                                   bottom="0.75"
                                   header="0.3"
                                   footer="0.3"/>
                   </worksheet>
"###,
r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0"
		xmlns:main="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
  xmlns:dot="http://www.martin-loetzsch.de/DOTML"
    exclude-result-prefixes="main">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="sheet-busn"/>
    <xsl:variable name="busndoc" select="document($sheet-busn)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="main:si" use="count(preceding-sibling::*)"/>
    <xsl:key name="row-by-id" match="main:row" use="key('string', main:c[1]/main:v)"/>

    <xsl:template match="/">
    <dot:graph
	   label="Data Lineage"
	   rankdir="LR"
	   nodesep="0.5">

      <xsl:apply-templates select="$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]"/>
      <dot:cluster id="legend" label="Legend" rankdir="TB">
	<dot:node id="legend_da" label="Data Asset"
      		shape="oval"
      		style="filled"
      		fillcolor="lightgreen"
      		color="green"
      		fontcolor="black">
      	</dot:node>
	<dot:node id="legend_prov" label="Provenance"
      		shape="diamond"
      		style="filled"
      		fillcolor="lightblue"
      		color="blue"
      		fontcolor="black">
      	</dot:node>
	<dot:node id="legend_external" label="External Source"
	      	shape="rectangle"
	      	style="filled"
	      	fillcolor="pink"
	      	color="red"
	      	fontcolor="black">
      	</dot:node>
	<dot:node id="legend_sink" label="Data Sink"
      		shape="tripleoctagon"
      		style="filled"
      		fillcolor="pink"
      		color="red"
      		fontcolor="black">
      	</dot:node>
      </dot:cluster>

      <xsl:apply-templates select='$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]' mode='arcs'/>
    </dot:graph>
    </xsl:template>

    <xsl:template match="main:row">
      <dot:node id="{key('string', main:c[1]/main:v, $root)}"
      	label="{key('string', main:c[2]/main:v, $root)}\n{key('string', main:c[1]/main:v, $root)}"
      	shape="oval"
      	style="filled"
      	fillcolor="lightgreen"
      	color="green"
      	fontcolor="black">
      </dot:node>
    </xsl:template>

    <xsl:template match='main:row' mode='arcs'>
    	<xsl:variable name="id" select="key('string', main:c[1]/main:v, $root)"/>
    	<xsl:for-each select="key('string',
    		key('row-by-id', $id, $techdoc)/main:c[3]/main:v, $root)">
          <xsl:if test="not(. = ('', 'none', 'n/a'))">
          	<dot:edge to='{$id}'
			from='{.}'
			color='black'>
          	</dot:edge>
          </xsl:if>
        </xsl:for-each>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("sheet-busn"), String::from("sheet1.xml"))],
        vec![(String::from("file:///path/to/resource/sheet1.xml"), String::from(
            r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{5B040C7D-E981-1C48-9EF2-327C94A0F723}">
                   <sheetData>
                      <row r="1" spans="1:3" ht="30" customHeight="1">
                         <c r="A1" s="10"/>
                         <c r="B1" s="10"/>
                         <c r="C1" s="10"/>
                      </row>
                      <row r="2" spans="1:3" ht="80.099999999999994" customHeight="1">
                         <c r="A2" s="8"/>
                         <c r="B2" s="11" t="s">
                            <v>0</v>
                         </c>
                         <c r="C2" s="8"/>
                      </row>
                      <row r="3" spans="1:3" ht="15" customHeight="1">
                         <c r="A3" s="8"/>
                         <c r="B3" s="8"/>
                         <c r="C3" s="8"/>
                      </row>
                    </sheetData>
</worksheet>"###))],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<dot:graph xmlns:dot='http://www.martin-loetzsch.de/DOTML' label='Data Lineage' nodesep='0.5' rankdir='LR'><dot:cluster id='legend' label='Legend' rankdir='TB'><dot:node color='green' fillcolor='lightgreen' fontcolor='black' id='legend_da' label='Data Asset' shape='oval' style='filled'/><dot:node color='blue' fillcolor='lightblue' fontcolor='black' id='legend_prov' label='Provenance' shape='diamond' style='filled'/><dot:node color='red' fillcolor='pink' fontcolor='black' id='legend_external' label='External Source' shape='rectangle' style='filled'/><dot:node color='red' fillcolor='pink' fontcolor='black' id='legend_sink' label='Data Sink' shape='tripleoctagon' style='filled'/></dot:cluster></dot:graph>"
    );
    Ok(())
}

pub fn variable_1<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{B4B4B9CF-A4A9-2146-844A-107C12CFFDC9}">
                   <sheetData>
                      <row r="1"
                           spans="1:15"
                           customFormat="1"
                           ht="39.950000000000003"
                           customHeight="1">
                         <c r="A1" s="23" t="s">
                            <v>25</v>
                         </c>
                         <c r="B1" s="112" t="s">
                            <v>95</v>
                         </c>
                         <c r="C1" s="112"/>
                         <c r="D1" s="112"/>
                         <c r="E1" s="112"/>
                         <c r="F1" s="112"/>
                         <c r="G1" s="112"/>
                         <c r="H1" s="112"/>
                         <c r="I1" s="112"/>
                         <c r="J1" s="112"/>
                         <c r="K1" s="112"/>
                         <c r="L1" s="112"/>
                         <c r="M1" s="23" t="s">
                            <v>2</v>
                         </c>
                      </row>
                      <row r="2" spans="1:15" ht="3.95" customHeight="1">
                         <c r="A2" s="25"/>
                         <c r="B2" s="25"/>
                         <c r="C2" s="25"/>
                         <c r="D2" s="25"/>
                         <c r="E2" s="25"/>
                         <c r="F2" s="25"/>
                         <c r="G2" s="25"/>
                         <c r="H2" s="25"/>
                         <c r="I2" s="25"/>
                         <c r="J2" s="25"/>
                         <c r="K2" s="25"/>
                         <c r="L2" s="25"/>
                         <c r="M2" s="25"/>
                         <c r="N2"/>
                         <c r="O2" s="5"/>
                      </row>
                      </sheetData>
                      <mergeCells count="7">
                         <mergeCell ref="L5:M5"/>
                         <mergeCell ref="B1:L1"/>
                         <mergeCell ref="A3:M3"/>
                         <mergeCell ref="A4:M4"/>
                         <mergeCell ref="B5:D5"/>
                         <mergeCell ref="E5:F5"/>
                         <mergeCell ref="G5:K5"/>
                      </mergeCells>
                      <pageMargins left="0.7"
                                   right="0.7"
                                   top="0.75"
                                   bottom="0.75"
                                   header="0.3"
                                   footer="0.3"/>
                   </worksheet>
"###,
r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0"
		xmlns:main="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
  xmlns:dot="http://www.martin-loetzsch.de/DOTML"
    exclude-result-prefixes="main">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="sheet-busn"/>
    <xsl:variable name="busndoc" select="document($sheet-busn)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="main:si" use="count(preceding-sibling::*)"/>
    <xsl:key name="row-by-id" match="main:row" use="key('string', main:c[1]/main:v)"/>

    <xsl:template match="/">
    <dot:graph
	   label="Data Lineage"
	   rankdir="LR"
	   nodesep="0.5">

      <xsl:apply-templates select="$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]"/>
      <xsl:for-each-group select="$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[2]/*]"
       group-by="main:c[2]/main:v">
         <xsl:if test="not(current-grouping-key() = ('', 'none'))">
           <xsl:variable name="prov-id" select="concat('prov_', current-grouping-key())"/>
           <dot:node id="{$prov-id}"
             label="{current-grouping-key()}"/>
           <xsl:for-each select="current-group()">
             <xsl:if test="main:c[1]/main:v">
               <dot:edge to="{main:c[1]/main:v}"
                 from="{$prov-id}"/>
             </xsl:if>
           </xsl:for-each>
         </xsl:if>
      </xsl:for-each-group>
      <xsl:apply-templates select='$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]' mode='arcs'/>
    </dot:graph>
    </xsl:template>

    <xsl:template match="main:row">
      <dot:node id="{key('string', main:c[1]/main:v, $root)}"
      	label="{key('string', main:c[2]/main:v, $root)}\n{key('string', main:c[1]/main:v, $root)}"
      	shape="oval"
      	style="filled"
      	fillcolor="lightgreen"
      	color="green"
      	fontcolor="black">
      </dot:node>
    </xsl:template>

    <xsl:template match='main:row' mode='arcs'>
    	<xsl:variable name="id" select="key('string', main:c[1]/main:v, $root)"/>
    	<xsl:for-each select="key('string',
    		key('row-by-id', $id, $techdoc)/main:c[3]/main:v, $root)">
          <xsl:if test="not(. = ('', 'none', 'n/a'))">
          	<dot:edge to='{$id}'
			from='{.}'
			color='black'>
          	</dot:edge>
          </xsl:if>
        </xsl:for-each>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("sheet-busn"), String::from("sheet1.xml"))],
        vec![(String::from("file:///path/to/resource/sheet1.xml"), String::from(
            r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{5B040C7D-E981-1C48-9EF2-327C94A0F723}">
                   <sheetData>
                      <row r="1" spans="1:3" ht="30" customHeight="1">
                         <c r="A1" s="10"/>
                         <c r="B1" s="10"><v>1</v></c>
                         <c r="C1" s="10"/>
                      </row>
                      <row r="2" spans="1:3" ht="80.099999999999994" customHeight="1">
                         <c r="A2" s="8"/>
                         <c r="B2" s="11" t="s">
                            <v>0</v>
                         </c>
                         <c r="C2" s="8"/>
                      </row>
                      <row r="3" spans="1:3" ht="15" customHeight="1">
                         <c r="A3" s="8"/>
                         <c r="B3" s="8"><v>1</v></c>
                         <c r="C3" s="8"/>
                      </row>
                    </sheetData>
</worksheet>"###))],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<dot:graph xmlns:dot='http://www.martin-loetzsch.de/DOTML' label='Data Lineage' nodesep='0.5' rankdir='LR'/>"
    );
    Ok(())
}

pub fn variable_2<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{B4B4B9CF-A4A9-2146-844A-107C12CFFDC9}">
                   <sheetData>
                      <row r="1"
                           spans="1:15"
                           customFormat="1"
                           ht="39.950000000000003"
                           customHeight="1">
                         <c r="A1" s="23" t="s">
                            <v>25</v>
                         </c>
                         <c r="B1" s="112" t="s">
                            <v>95</v>
                         </c>
                         <c r="C1" s="112"/>
                         <c r="D1" s="112"/>
                         <c r="E1" s="112"/>
                         <c r="F1" s="112"/>
                         <c r="G1" s="112"/>
                         <c r="H1" s="112"/>
                         <c r="I1" s="112"/>
                         <c r="J1" s="112"/>
                         <c r="K1" s="112"/>
                         <c r="L1" s="112"/>
                         <c r="M1" s="23" t="s">
                            <v>2</v>
                         </c>
                      </row>
                      <row r="2" spans="1:15" ht="3.95" customHeight="1">
                         <c r="A2" s="25"/>
                         <c r="B2" s="25"/>
                         <c r="C2" s="25"/>
                         <c r="D2" s="25"/>
                         <c r="E2" s="25"/>
                         <c r="F2" s="25"/>
                         <c r="G2" s="25"/>
                         <c r="H2" s="25"/>
                         <c r="I2" s="25"/>
                         <c r="J2" s="25"/>
                         <c r="K2" s="25"/>
                         <c r="L2" s="25"/>
                         <c r="M2" s="25"/>
                         <c r="N2"/>
                         <c r="O2" s="5"/>
                      </row>
                      </sheetData>
                      <mergeCells count="7">
                         <mergeCell ref="L5:M5"/>
                         <mergeCell ref="B1:L1"/>
                         <mergeCell ref="A3:M3"/>
                         <mergeCell ref="A4:M4"/>
                         <mergeCell ref="B5:D5"/>
                         <mergeCell ref="E5:F5"/>
                         <mergeCell ref="G5:K5"/>
                      </mergeCells>
                      <pageMargins left="0.7"
                                   right="0.7"
                                   top="0.75"
                                   bottom="0.75"
                                   header="0.3"
                                   footer="0.3"/>
                   </worksheet>
"###,
r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0"
		xmlns:main="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
  xmlns:dot="http://www.martin-loetzsch.de/DOTML"
    exclude-result-prefixes="main">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="sheet-busn"/>
    <xsl:variable name="busndoc" select="document($sheet-busn)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="main:si" use="count(preceding-sibling::*)"/>
    <xsl:key name="row-by-id" match="main:row" use="key('string', main:c[1]/main:v)"/>

    <xsl:template match="/">
    <dot:graph
	   label="Data Lineage"
	   rankdir="LR"
	   nodesep="0.5">

      <xsl:apply-templates select="$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]"/>
      <xsl:for-each select="$busndoc/main:worksheet/main:sheetData/main:row">
        <xsl:variable name="this" select="."/>
        <xsl:for-each select="(1 to 5)">
           <dot:node id="{name($this)}-{.}"
             label="name {name($this)}"/>
        </xsl:for-each>
      </xsl:for-each>
      <xsl:apply-templates select='$busndoc/main:worksheet/main:sheetData/main:row[position() gt 6][main:c[1]/*]' mode='arcs'/>
    </dot:graph>
    </xsl:template>

    <xsl:template match="main:row">
      <dot:node id="{key('string', main:c[1]/main:v, $root)}"
      	label="{key('string', main:c[2]/main:v, $root)}\n{key('string', main:c[1]/main:v, $root)}"
      	shape="oval"
      	style="filled"
      	fillcolor="lightgreen"
      	color="green"
      	fontcolor="black">
      </dot:node>
    </xsl:template>

    <xsl:template match='main:row' mode='arcs'>
    	<xsl:variable name="id" select="key('string', main:c[1]/main:v, $root)"/>
    	<xsl:for-each select="key('string',
    		key('row-by-id', $id, $techdoc)/main:c[3]/main:v, $root)">
          <xsl:if test="not(. = ('', 'none', 'n/a'))">
          	<dot:edge to='{$id}'
			from='{.}'
			color='black'>
          	</dot:edge>
          </xsl:if>
        </xsl:for-each>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("sheet-busn"), String::from("sheet1.xml"))],
        vec![(String::from("file:///path/to/resource/sheet1.xml"), String::from(
            r###"<?xml version="1.0" encoding="ascii"?>
        <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                   xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
                   xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                   xmlns:x14ac="http://schemas.microsoft.com/office/spreadsheetml/2009/9/ac"
                   xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"
                   xmlns:xr2="http://schemas.microsoft.com/office/spreadsheetml/2015/revision2"
                   xmlns:xr3="http://schemas.microsoft.com/office/spreadsheetml/2016/revision3"
                   mc:Ignorable="x14ac xr xr2 xr3"
                   xr:uid="{5B040C7D-E981-1C48-9EF2-327C94A0F723}">
                   <sheetData>
                      <row r="1" spans="1:3" ht="30" customHeight="1">
                         <c r="A1" s="10"/>
                         <c r="B1" s="10"><v>1</v></c>
                         <c r="C1" s="10"/>
                      </row>
                      <row r="2" spans="1:3" ht="80.099999999999994" customHeight="1">
                         <c r="A2" s="8"/>
                         <c r="B2" s="11" t="s">
                            <v>0</v>
                         </c>
                         <c r="C2" s="8"/>
                      </row>
                      <row r="3" spans="1:3" ht="15" customHeight="1">
                         <c r="A3" s="8"/>
                         <c r="B3" s="8"><v>1</v></c>
                         <c r="C3" s="8"/>
                      </row>
                    </sheetData>
</worksheet>"###))],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<dot:graph xmlns:dot='http://www.martin-loetzsch.de/DOTML' label='Data Lineage' nodesep='0.5' rankdir='LR'><dot:node id='row-1' label='name row'/><dot:node id='row-2' label='name row'/><dot:node id='row-3' label='name row'/><dot:node id='row-4' label='name row'/><dot:node id='row-5' label='name row'/><dot:node id='row-1' label='name row'/><dot:node id='row-2' label='name row'/><dot:node id='row-3' label='name row'/><dot:node id='row-4' label='name row'/><dot:node id='row-5' label='name row'/><dot:node id='row-1' label='name row'/><dot:node id='row-2' label='name row'/><dot:node id='row-3' label='name row'/><dot:node id='row-4' label='name row'/><dot:node id='row-5' label='name row'/></dot:graph>"
    );
    Ok(())
}

// Process nodes in one document and look up a key in another document
pub fn doc_key_2<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<doc>
    <row>
        <column>
            <code>1</code>
            <value>one</value>
        </column>
        <column>
            <code>2</code>
            <value>two</value>
        </column>
        <column>
            <code>3</code>
            <value>three</value>
        </column>
        <column>
            <code>4</code>
            <value>four</value>
        </column>
    </row>
</doc>
"###,
        r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="ext"/>
    <xsl:variable name="extdoc" select="document($ext)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="mykey" match="column" use="code"/>

    <xsl:template match="/">
      <result>
        <xsl:apply-templates select="$extdoc/main/data"/>
      </result>
    </xsl:template>

    <xsl:template match="main/data">
      <out value="{key('mykey', class, $root)/value}"/>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("ext"), String::from("external1.xml"))],
        vec![(
            String::from("file:///path/to/resource/external1.xml"),
            String::from(
                r###"<main>
                   <data>
                      <class>4</class>
                    </data>
                    <data>
                      <class>1</class>
                    </data>
                    <data>
                      <class>2</class>
                    </data>
                    <data>
                      <class>4</class>
                    </data>
                    <data>
                      <class>3</class>
                    </data>
                    <data>
                      <class>5</class>
                    </data>
                    <data>
                      <class>3</class>
                    </data>
</main>"###,
            ),
        )],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<result><out value='four'/><out value='one'/><out value='two'/><out value='four'/><out value='three'/><out value=''/><out value='three'/></result>"
    );
    Ok(())
}

// Simulate SpreadsheetML document
pub fn doc_key_3<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<doc>
    <si>
      <t>one</t>
    </si>
    <si>
      <t>two</t>
    </si>
    <si>
      <t>three</t>
    </si>
    <si>
      <t>four</t>
    </si>
</doc>
"###,
        r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="ext"/>
    <xsl:variable name="extdoc" select="document($ext)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="si" use="count(preceding-sibling::*) + 1"/>

    <xsl:template match="/">
      <result>
        <xsl:apply-templates select="$extdoc/main/data"/>
      </result>
    </xsl:template>

    <xsl:template match="main/data">
      <xsl:message>looking up <xsl:sequence select="i[1]"/></xsl:message>
      <out value="{key('string', i[1], $root)/t}"/>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("ext"), String::from("external1.xml"))],
        vec![(
            String::from("file:///path/to/resource/external1.xml"),
            String::from(
                r###"<main>
                   <data>
                      <i>4</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>1</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>2</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>4</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>3</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>5</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>3</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
</main>"###,
            ),
        )],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<result><out value='four'/><out value='one'/><out value='two'/><out value='four'/><out value='three'/><out value=''/><out value='three'/></result>"
    );
    Ok(())
}

// Lookup key in an externally loaded document
pub fn doc_key_4<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<doc>
<si>
  <t>1</t>
</si>
<si>
  <t>2</t>
</si>
<si>
  <t>3</t>
</si>
<si>
  <t>4</t>
</si>
</doc>
"###,
        r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="ext"/>
    <xsl:variable name="extdoc" select="document($ext)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="idx" match="data" use="i"/>

    <xsl:template match="/">
      <result>
        <xsl:apply-templates select="/doc/si"/>
      </result>
    </xsl:template>

    <xsl:template match="doc/si">
      <xsl:message>looking up <xsl:value-of select="t"/></xsl:message>
      <out index="{t}" value="{count(key('idx', t, $extdoc)/d)}"/>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("ext"), String::from("external1.xml"))],
        vec![(
            String::from("file:///path/to/resource/external1.xml"),
            String::from(
                r###"<main>
                   <data>
                      <i>4</i>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>1</i>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>2</i>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>4</i>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>3</i>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>5</i>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
                    <data>
                      <i>3</i>
                      <d>something else</d>
                      <d>something else</d>
                      <d>something else</d>
                    </data>
</main>"###,
            ),
        )],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<result><out index='1' value='1'/><out index='2' value='2'/><out index='3' value='6'/><out index='4' value='8'/></result>"
    );
    Ok(())
}

// Lookup key the definition of another key
pub fn doc_key_5<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<doc>
<si>
  <t>zero</t>
</si>
<si>
  <t>one</t>
</si>
<si>
  <t>two</t>
</si>
<si>
  <t>three</t>
</si>
<si>
  <t>four</t>
</si>
</doc>
"###,
        r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
		version="2.0">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="ext"/>
    <xsl:variable name="extdoc" select="document($ext)"/>
    <xsl:param name="busn"/>
    <xsl:variable name="busndoc" select="document($busn)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="si" use="count(preceding-sibling::si)"/>
    <xsl:key name="by-id" match="data" use="key('string', idx, $root)"/>

    <xsl:template match="/">
      <result>
        <xsl:apply-templates select="$busndoc/main/data"/>
      </result>
    </xsl:template>

    <xsl:template match="data"> <!-- in busn doc -->
      <xsl:message>looking up <xsl:value-of select="r"/></xsl:message>
      <out lookup="{r}"><xsl:value-of select="key('by-id', r, $extdoc)/v"/></out>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![
            (String::from("ext"), String::from("external1.xml")),
            (String::from("busn"), String::from("business.xml")),
        ],
        vec![
            (
                String::from("file:///path/to/resource/external1.xml"),
                String::from(
                    r###"<main>
                   <data>
                      <v>vvvv</v>
                      <idx>4</idx>
                    </data>
                    <data>
                      <v>v</v>
                      <idx>1</idx>
                    </data>
                    <data>
                      <v>vv</v>
                      <idx>2</idx>
                    </data>
                    <data>
                      <v>vvvv</v>
                      <idx>4</idx>
                    </data>
                    <data>
                      <v>vvv</v>
                      <idx>3</idx>
                    </data>
                    <data>
                      <v>vvvvv</v>
                      <idx>5</idx>
                    </data>
                    <data>
                      <v>vvv</v>
                      <idx>3</idx>
                    </data>
</main>"###,
                ),
            ),
            (
                String::from("file:///path/to/resource/business.xml"),
                String::from(
                    r###"<main>
               <data>
                  <r>four</r>
                  <s>ssss</s>
                </data>
                <data>
                  <r>one</r>
                  <s>s</s>
                </data>
                <data>
                  <r>two</r>
                  <s>ss</s>
                </data>
                <data>
                  <r>four</r>
                  <s>ssss</s>
                </data>
                <data>
                  <r>three</r>
                  <s>sss</s>
                </data>
                <data>
                  <r>five</r>
                  <s>sssss</s>
                </data>
                <data>
                  <r>three</r>
                  <s>sss</s>
                </data>
</main>"###,
                ),
            ),
        ],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<result><out lookup='four'>vvvvvvvv</out><out lookup='one'>v</out><out lookup='two'>vv</out><out lookup='four'>vvvvvvvv</out><out lookup='three'>vvvvvv</out><out lookup='five'></out><out lookup='three'>vvvvvv</out></result>"
    );
    Ok(())
}

pub fn avt_1<N: Node, G, I, J>(
    parse_from_str: G,
    parse_from_str_with_ns: J,
    make_doc: I,
) -> Result<(), Error>
where
    G: Fn(&str) -> Result<N, Error>,
    I: Fn() -> Result<N, Error>,
    J: Fn(&str) -> Result<(N, Option<NamespaceMap>), Error>,
{
    let result = test_rig_with_params(
        r###"<doc>
    <si>
      <t>one</t>
    </si>
    <si>
      <t>two</t>
    </si>
    <si>
      <t>three</t>
    </si>
    <si>
      <t>four</t>
    </si>
</doc>
"###,
        r###"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
xmlns:dot="http://www.martin-loetzsch.de/DOTML"
xmlns:test="urn:test-org"
		version="2.0">

    <xsl:output method="xml" indent="yes"/>
    <xsl:strip-space elements="*"/>

    <xsl:param name="ext"/>
    <xsl:variable name="extdoc" select="document($ext)"/>

    <xsl:variable name="root" select="/"/>

    <xsl:key name="string" match="si" use="count(preceding-sibling::*) + 1"/>

    <xsl:template match="/">
      <dot:graph>
        <xsl:apply-templates select="$extdoc/test:main/test:data"/>
      </dot:graph>
    </xsl:template>

    <xsl:template match="test:main/test:data">
      <xsl:message>looking up <xsl:sequence select="test:i[1]"/></xsl:message>
      <dot:node id="{key('string', test:i[1], $root)/t}"/>
    </xsl:template>
</xsl:stylesheet>
"###,
        parse_from_str,
        parse_from_str_with_ns,
        make_doc,
        Some(Url::parse("file:///path/to/resource/primary.xml").expect("unable to parse URL")),
        vec![(String::from("ext"), String::from("external1.xml"))],
        vec![(
            String::from("file:///path/to/resource/external1.xml"),
            String::from(
                r###"<main xmlns="urn:test-org">
                   <data>
                      <i>4</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>1</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>2</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>4</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>3</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>5</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
                    <data>
                      <i>3</i>
                      <i>something else</i>
                      <i>something else</i>
                    </data>
</main>"###,
            ),
        )],
    )
    .expect("transform failed");

    assert_eq!(result.0.len(), 1);
    assert_eq!(
        result.1.to_xml(),
        "<dot:graph xmlns:dot='http://www.martin-loetzsch.de/DOTML'><dot:node id='four'/><dot:node id='one'/><dot:node id='two'/><dot:node id='four'/><dot:node id='three'/><dot:node id=''/><dot:node id='three'/></dot:graph>"
    );
    Ok(())
}
