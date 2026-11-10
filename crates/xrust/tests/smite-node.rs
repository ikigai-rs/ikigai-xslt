use xrust::trees::smite::RNode;

mod node;
mod smite;

// Item Node tests

#[test]
fn node_get_attr_node() {
    node::get_attr_node::<RNode, _>(smite::make_empty_doc).expect("test failed")
}
#[test]
fn node_to_xml_special_1() {
    node::to_xml_special_1::<RNode, _>(smite::make_empty_doc).expect("test failed")
}
#[test]
fn node_to_xml_special_2() {
    node::to_xml_special_2::<RNode, _>(smite::make_empty_doc).expect("test failed")
}
#[test]
fn node_descend_1() {
    node::descend_1::<RNode, _>(smite::make_empty_doc).expect("test failed")
}
