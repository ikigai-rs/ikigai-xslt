use qualname::QName;
use xrust::item::{Node, NodeType};
use xrust::item_node_tests;
use xrust::item_value_tests;
use xrust::trees::smite::RNode;

mod smite;

item_value_tests!(RNode);

// Item Node tests

item_node_tests!(smite::make_empty_doc, smite::make_doc, smite::make_sd_raw);
