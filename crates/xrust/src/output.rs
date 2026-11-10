/*! How to serialise a tree structure.
*/

use core::fmt;
use qualname::{NcName, QName};

/// An output definition. See XSLT v3.0 26 Serialization
#[derive(Clone, Debug)]
pub struct OutputDefinition {
    name: Option<QName>, // TODO: EQName
    indent: bool,
    method: Method,
    // TODO: all the other myriad output parameters
}

impl Default for OutputDefinition {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputDefinition {
    pub fn new() -> OutputDefinition {
        OutputDefinition {
            name: None,
            indent: false,
            method: Method::Xml,
        }
    }
    pub fn get_name(&self) -> Option<QName> {
        self.name.clone()
    }
    pub fn set_name(&mut self, name: Option<QName>) {
        match name {
            Some(n) => {
                self.name.replace(n);
            }
            None => {
                self.name = None;
            }
        }
    }
    pub fn get_indent(&self) -> bool {
        self.indent
    }
    pub fn set_indent(&mut self, ind: bool) {
        self.indent = ind;
    }
    pub fn method(&self) -> Method {
        self.method.clone()
    }
    pub fn set_method(&mut self, m: Method) {
        self.method = m;
    }
}
impl fmt::Display for OutputDefinition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.indent {
            f.write_str("indent output")
        } else {
            f.write_str("do not indent output")
        }
    }
}

/// Serialisation method
#[derive(Clone, Debug, PartialEq)]
pub enum Method {
    Xml,
    Text,
    Html,
    Other(QName),
}

impl From<String> for Method {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "xml" => Method::Xml,
            "text" => Method::Text,
            "html" => Method::Html,
            _ => Method::Other(QName::from_local_name(
                NcName::try_from(value.as_str()).expect("not a valid method"),
            )),
        }
    }
}

/// Directive for how to treat a [Value](crate::value::Value) upon serialisation.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputSpec {
    Normal,
    Escaped,
    NoEscape,
}
