//! Support for keys.

use crate::item::{Node, Sequence};
use crate::transform::Transform;
use crate::transform::context::{Context, ContextBuilder, StaticContext};
use crate::xdmerror::Error;
use crate::{Item, SequenceTrait};
use std::collections::HashMap;
use url::Url;

/// For the named key:
/// 1. find the nodes in the document that match the pattern
/// 2. Evaluate the expression to calculate the key value
/// 3. Store the key value -> Node mapping
///     NB. an optimisation is to calculate a key's value the first time that key is accessed for a document
///     TODO: support composite keys
pub(crate) fn populate_key_values<
    N: Node,
    F: FnMut(&str) -> Result<(), Error>,
    G: FnMut(&str) -> Result<N, Error>,
    H: FnMut(&Url) -> Result<String, Error>,
>(
    ctxt: &Context<N>,
    stctxt: &mut StaticContext<N, F, G, H>,
    sd: N,
    key_name: &str,
) -> Result<(), Error> {
    let root = sd.owner_document();
    let sdid = root.get_id();
    let keyid = make_key(key_name, sdid.clone());

    // Find the declaration of this key in the context
    let decls = ctxt.keys.get(key_name).map_or(
        Err(Error::new(
            crate::ErrorKind::StaticAbsent,
            format!("no declaration of key \"{}\"", key_name),
        )),
        |d| Ok(d),
    )?;

    for n in root.descend_iter() {
        // Descend visits all nodes except attributes
        // TODO: support attributes
        for (m, u) in decls {
            if m.matches(ctxt, stctxt, &Item::Node(n.clone())) {
                // Calculate the key values for this node.
                // There may be multiple values
                let newctxt = ContextBuilder::from(&*ctxt)
                    .context(vec![Item::Node(n.clone())])
                    .build();
                let values_res = newctxt.dispatch(stctxt, u);
                if values_res.is_err() {
                    return Err(values_res.unwrap_err());
                }
                let values = values_res.unwrap();
                // Each item in values is a value for this key
                values.iter().for_each(|v| {
                    if let Some(kv) = stctxt.key_values.get_mut(&keyid) {
                        // We've already seen this value, so append to existing mapping
                        if let Some(vv) = kv.get_mut(&v.to_string()) {
                            // This value for this key already has a mapping, so append this node
                            vv.push(n.clone());
                        } else {
                            // This value for this key has not been seen before, so create new mapping
                            kv.insert(v.to_string(), vec![n.clone()]);
                        }
                    } else {
                        // Haven't seen this key before, so create new mapping
                        let mut new = HashMap::new();
                        new.insert(v.to_string(), vec![n.clone()]);
                        stctxt.key_values.insert(keyid.clone(), new);
                    }
                })
            }
        }
    }

    // Flag this key as being populated
    stctxt.key_populated.insert(keyid, true);

    Ok(())
}

/// Look up the value of a key in a document. The value is evaluated to a Sequence. The interpretation of the sequence depends on the key's composite setting.
/// Key values are lazily calculated. Check whether this key has been calculated for this document,
/// and populate if required.
/// TODO: support composite keys
pub fn key<
    N: Node,
    F: FnMut(&str) -> Result<(), Error>,
    G: FnMut(&str) -> Result<N, Error>,
    H: FnMut(&Url) -> Result<String, Error>,
>(
    ctxt: &Context<N>,
    stctxt: &mut StaticContext<N, F, G, H>,
    name: &Box<Transform<N>>,
    v: &Box<Transform<N>>,
    doc: &Box<Transform<N>>,
) -> Result<Sequence<N>, Error> {
    let keydoc = ctxt.dispatch(stctxt, doc)?;
    if keydoc.len() == 1 {
        if let Item::Node(kd) = &keydoc[0] {
            let keyname = ctxt.dispatch(stctxt, name)?.to_string();
            let keyid = make_key(keyname.as_str(), kd.get_id());
            // Has this (key,doc) combo been populated?
            // If not then do it now.

            let pop = stctxt.key_populated.get(&keyid).unwrap_or(&false);
            if !pop {
                populate_key_values(ctxt, stctxt, kd.clone(), keyname.as_str())?;
            }

            Ok(ctxt.dispatch(stctxt, v)?.iter().fold(vec![], |mut acc, s| {
                if let Some(u) = stctxt.key_values.get(&keyid) {
                    if let Some(a) = u.get(&s.to_string()) {
                        let mut b: Sequence<N> = a.iter().map(|n| Item::Node(n.clone())).collect();
                        acc.append(&mut b);
                        acc
                    } else {
                        acc
                    }
                } else {
                    acc
                }
            }))
        } else {
            return Err(Error::new(
                crate::ErrorKind::TypeError,
                "document must be a document-type node",
            ));
        }
    } else {
        return Err(Error::new(
            crate::ErrorKind::TypeError,
            "document must be a singleton sequence",
        ));
    }
}

// Creates a string key for use as a HaspMap index, combining key name with document ID
pub(crate) fn make_key(key_name: &str, doc_id: String) -> String {
    format!("{}^{}", key_name, doc_id)
}
