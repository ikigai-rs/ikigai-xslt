use qualname::NamespacePrefix;
use xrust::parser::ParseError;

pub fn missing(prefix: &NamespacePrefix) -> ParseError {
    ParseError::MissingNameSpace {
        prefix: Some(prefix.to_string()),
        nsuri: None,
        message: None,
    }
}
