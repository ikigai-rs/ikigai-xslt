//! Support for variables.

use crate::item::{Node, Sequence};
use crate::transform::Transform;
use crate::transform::context::{Context, ContextBuilder, StaticContext};
use crate::xdmerror::{Error, ErrorKind};
use url::Url;

/// Declare a variable in a new scope and then evaluate the given transformation.
/// Returns the result of the transformation.
pub fn declare_variable<
    N: Node,
    F: FnMut(&str) -> Result<(), Error>,
    G: FnMut(&str) -> Result<N, Error>,
    H: FnMut(&Url) -> Result<String, Error>,
>(
    ctxt: &Context<N>,
    stctxt: &mut StaticContext<N, F, G, H>,
    name: String,
    value: &Transform<N>,
    f: &Transform<N>,
) -> Result<Sequence<N>, Error> {
    ContextBuilder::from(ctxt)
        .variable(name, ctxt.dispatch(stctxt, value)?)
        .build()
        .dispatch(stctxt, f)
}

/// Define a variable in the current context and scope.
/// The value of the variable is the result of evaluating the given transformation.
/// The result is an empty sequence (XSLT 3.0 s9.8).
pub fn set_variable<
    N: Node,
    F: FnMut(&str) -> Result<(), Error>,
    G: FnMut(&str) -> Result<N, Error>,
    H: FnMut(&Url) -> Result<String, Error>,
>(
    ctxt: &Context<N>,
    stctxt: &mut StaticContext<N, F, G, H>,
    name: String,
    value: &Transform<N>,
) -> Result<Sequence<N>, Error> {
    let varval = ctxt.dispatch(stctxt, value)?;
    ctxt.var_push(name, varval);
    Ok(vec![])
}

pub fn reference_variable<N: Node>(ctxt: &Context<N>, name: &String) -> Result<Sequence<N>, Error> {
    ctxt.var_value(name).map_or_else(
        || {
            Err(Error::new(
                ErrorKind::Unknown,
                format!("unknown variable \"{}\"", name),
            ))
        },
        |u| Ok(u.clone()),
    )
}
