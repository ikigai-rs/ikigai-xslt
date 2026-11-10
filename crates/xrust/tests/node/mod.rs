//! Generic node tests

use qualname::{NcName, QName};
use std::rc::Rc;
use xrust::item::{Node, NodeType};
use xrust::output::OutputSpec;
use xrust::value::{Value, ValueBuilder, ValueData};
use xrust::xdmerror::Error;

pub fn get_attr_node<N: Node, G>(make_doc: G) -> Result<(), Error>
where
    G: Fn() -> N,
{
    let mut sd = make_doc();
    let t = sd.new_element(QName::from_local_name(NcName::try_from("Test").unwrap()))?;
    sd.push(t.clone())?;
    let a1 = sd.new_attribute(
        QName::from_local_name(NcName::try_from("role").unwrap()),
        Rc::new(Value::from("testing")),
    )?;
    t.add_attribute(a1)?;
    let a2 = sd.new_attribute(
        QName::from_local_name(NcName::try_from("phase").unwrap()),
        Rc::new(Value::from("one")),
    )?;
    t.add_attribute(a2)?;

    // NB. attributes could be returned in a different order
    assert!(
        sd.to_xml() == "<Test role='testing' phase='one'/>"
            || sd.to_xml() == "<Test phase='one' role='testing'/>"
    );

    match t.get_attribute_node(&QName::from_local_name(NcName::try_from("role").unwrap())) {
        Some(at) => {
            assert_eq!(at.node_type(), NodeType::Attribute);
            assert_eq!(at.to_string(), "testing");
            Ok(())
        }
        None => panic!("unable to find attribute \"role\""),
    }
}

pub fn to_xml_special_1<N: Node, G>(make_doc: G) -> Result<(), Error>
where
    G: Fn() -> N,
{
    let mut sd = make_doc();
    let mut t = sd.new_element(QName::from_local_name(NcName::try_from("Test").unwrap()))?;
    sd.push(t.clone())?;
    let a1 = sd.new_attribute(
        QName::from_local_name(NcName::try_from("attr").unwrap()),
        Rc::new(Value::from("'")),
    )?;
    t.add_attribute(a1)?;
    let t1 = sd
        .new_text(Rc::new(Value::from(
            r##"
        XML escape test: < > & ' "
"##,
        )))
        .expect("unable to create text node");
    t.push(t1).expect("unable to add text node");
    assert_eq!(
        t.to_xml(),
        r##"<Test attr='&apos;'>
        XML escape test: &lt; &gt; &amp; &apos; &quot;
</Test>"##
    );
    Ok(())
}

pub fn to_xml_special_2<N: Node, G>(make_doc: G) -> Result<(), Error>
where
    G: Fn() -> N,
{
    let mut sd = make_doc();
    let mut t = sd.new_element(QName::from_local_name(NcName::try_from("Test").unwrap()))?;
    sd.push(t.clone())?;
    let a1 = sd.new_attribute(
        QName::from_local_name(NcName::try_from("attr").unwrap()),
        Rc::new(Value::from("'")),
    )?;
    t.add_attribute(a1)?;
    let t1 = sd
        .new_text(Rc::new(
            ValueBuilder::new()
                .value(ValueData::String(String::from(
                    r##"
        XML escape test: < > & ' "
"##,
                )))
                .output(OutputSpec::NoEscape)
                .build(),
        ))
        .expect("unable to create text node");
    t.push(t1).expect("unable to add text node");
    assert_eq!(
        t.to_xml(),
        r##"<Test attr='&apos;'>
        XML escape test: < > & ' "
</Test>"##
    );
    Ok(())
}

pub fn descend_1<N: Node, G>(make_doc: G) -> Result<(), Error>
where
    G: Fn() -> N,
{
    let mut sd = make_doc();
    let mut t = sd.new_element(QName::from_local_name(NcName::try_from("Test").unwrap()))?;
    sd.push(t.clone())?;

    let mut l1 = sd.new_element(QName::from_local_name(NcName::try_from("Level-1").unwrap()))?;
    t.push(l1.clone())?;
    let l1_t = sd.new_text(Rc::new(Value::from("first Level-1")))?;
    l1.push(l1_t).expect("unable to add node");
    let mut l1_1 = sd.new_element(QName::from_local_name(NcName::try_from("Level-2").unwrap()))?;
    l1.push(l1_1.clone())?;
    let l1_1_t = sd.new_text(Rc::new(Value::from("first Level-1 first Level-2")))?;
    l1_1.push(l1_1_t).expect("unable to add node");
    let mut l1_2 = sd.new_element(QName::from_local_name(NcName::try_from("Level-2").unwrap()))?;
    l1.push(l1_2.clone())?;
    let l1_2_t = sd.new_text(Rc::new(Value::from("first Level-1 second Level-2")))?;
    l1_2.push(l1_2_t).expect("unable to add node");

    let mut l2 = sd.new_element(QName::from_local_name(NcName::try_from("Level-1").unwrap()))?;
    t.push(l2.clone())?;
    let l2_t = sd.new_text(Rc::new(Value::from("second Level-1")))?;
    l2.push(l2_t).expect("unable to add node");
    let mut l2_1 = sd.new_element(QName::from_local_name(NcName::try_from("Level-2").unwrap()))?;
    l2.push(l2_1.clone())?;
    let l2_1_t = sd.new_text(Rc::new(Value::from("second Level-1 first Level-2")))?;
    l2_1.push(l2_1_t).expect("unable to add node");
    let mut l2_2 = sd.new_element(QName::from_local_name(NcName::try_from("Level-2").unwrap()))?;
    l2.push(l2_2.clone())?;
    let l2_2_t = sd.new_text(Rc::new(Value::from("second Level-1 second Level-2")))?;
    l2_2.push(l2_2_t).expect("unable to add node");

    assert_eq!(
        sd.to_xml(),
        "<Test><Level-1>first Level-1<Level-2>first Level-1 first Level-2</Level-2><Level-2>first Level-1 second Level-2</Level-2></Level-1><Level-1>second Level-1<Level-2>second Level-1 first Level-2</Level-2><Level-2>second Level-1 second Level-2</Level-2></Level-1></Test>"
    );

    assert_eq!(sd.descend_iter().count(), 13);
    Ok(())
}
