> **This is `ikigai-xrust` 2.3.0-ikigai.2, not xrust.** It is xrust 2.3.0, as published on
> crates.io, with the fixes below, carried by
> [ikigai-xslt](https://github.com/ikigai-rs/ikigai-xslt) until a release of xrust includes
> them. Each is drafted for upstream, as issue text and a patch, in that repository's
> `docs/upstream/` (its `README.md` gives the order the patches apply in), with a test here
> that fails on 2.3.0:
>
> - the XML parser keeps every prefixed attribute value, and `xml:id`, as written; 2.3.0
>   lowercased them (`src/parser/xml/attribute.rs`; 2.3.0-ikigai.1, ledger #3342);
> - whitespace-only `xsl:text` is kept; 2.3.0 stripped it with the stylesheet's
>   indentation (`src/transform/whitespace.rs`; ledger #1693);
> - a boolean's string value is `true` or `false`, and its number 1 or 0; 2.3.0 gave the
>   empty string and NaN (`src/value.rs`; #2520, #2668);
> - a comparison with a node on its left mirrors its operator; 2.3.0 evaluated `/r/v > 2`
>   as `2 > 3` (`src/item.rs`; #2668);
> - a character reference to a tab, LF or CR in an attribute keeps its character, and
>   attribute values are not collapsed; 2.3.0 made them spaces
>   (`src/parser/xml/attribute.rs`; #2520);
> - `number()` and arithmetic convert a node by its string value, any numeric type by its
>   lexical form, and a sequence by its first item; 2.3.0 gave NaN
>   (`src/item.rs`, `src/value.rs`, `src/transform/numbers.rs`; #2487);
> - a literal result element declares the namespaces of its attributes' names; 2.3.0
>   panicked serializing them (`src/xslt.rs`; #2520).
>
> - `xsl:text` stores characters and escapes only during serialization; 2.3.0 escaped
>   special characters twice (`src/xslt.rs`; #3493).
>
> No public API changes. The library is still named `xrust`, so code that
> uses it does not change. It is licensed, like xrust, under the Apache License 2.0
> ([LICENSE](./LICENSE)); xrust 2.3.0 ships no NOTICE file. The packaging differs from
> 2.3.0's in four ways that do not touch the library: the manifest names the package
> `ikigai-xrust`, leaves out the `ixml` example (it needs `earleybird`, a git dependency the
> published manifest does not carry), and allows clippy and rustdoc lints; `rustfmt.toml`
> turns formatting off; and the conformance-suite data and `tests/xml/45M.xml`, read only by
> feature-gated or commented-out tests, are left out.

# χrust

[![crates.io](https://img.shields.io/crates/v/xrust.svg)](https://crates.io/crates/xrust)
[![Released API docs](https://docs.rs/xrust/badge.svg)](https://docs.rs/xrust)
[![ALv2 licensed](https://img.shields.io/badge/license-ALv2-blue.svg)](./LICENSE)

XPath, XQuery, and XSLT for Rust

Pronounced "crust".

The goal of this project is to implement [XPath 3.1](https://www.w3.org/TR/xpath-31/), [XQuery 3.1](https://www.w3.org/TR/xquery-31/) and [XSLT 3.0](http://www.w3.org/TR/xslt-30/) in Rust.
Also included is an XML parser, using a parser combinator inspired by Nom.

Currently, the project has achieved the functional equivalent of XPath 1.0 and XSLT 1.0. That is, all of the elements and functions in v1.0 XPath and XSLT have been implemented.
However, it is not *compliant* with XPath/XSLT v1.0. This is because it implements the v3.0 data model.

In addition to the (rudimentary) implementation of the [XQuery and XPath Data Model 3.1](https://www.w3.org/TR/xpath-datamodel-31/) data model, a few other features of XPath and XSLT 2.0/3.0 have been implemented, such as FLWR expressions, grouping (i.e. xsl:for-each-group), and user-defined functions.

## Parent Group

χrust is part of the [markup-rs](https://gitlab.gnome.org/World/Rust/markup-rs) family of projects providing support for the XML tech stack in Rust.

The various modules in χrust are being slowly picked apart and moved into separate projects in the markup-rs group. The [Qualified Name](https://gitlab.gnome.org/World/Rust/markup-rs/qualname) module was the first to undergo this transition, and is now the [qualname](https://crates.io/crates/qualname) crate.

## Canonical Repository

Development of χrust takes place on [Gitlab](https://gitlab.gnome.org/World/Rust/markup-rs/xrust). The Gitlab repository is mirrored with [Github](https://github.com/ballsteve/xrust).

## Version 2.0 Release Notes

When building χrust v2.0 we have noticed unusually long compile times. If your application is adversely affected by this then please let us know. We're working on identifying the cause and developing a solution.

## Design

The library separates parsing from evaluation. XPath expressions and XSLT stylesheets are parsed (or "compiled") into an internal representation, a "transformation", which is then evaluated.

This means it won't matter how an expression is written; it will be compiled into the same internal form. For example, the XPath expression:

```xpath
if $a then "a is true" else ""
```

will result in the same internal format as:

```xml
<xsl:if test="$a">
  a is true
</xsl:if>
```

This approach means that the XPath and XSLT modules are simply mappings from their respective syntaxes to an χrust transformation.

### Trees

The "Tree" is the fundamental data object for transformations.
χrust provides a tree implementation, smite, that is both mutable and fully navigable.
By "fully navigable" we mean that from any given node you can access its children, parent, siblings, or attributes.
It achieves mutability by using the interior mutability pattern ("smite" is an anagram of "tuple struct enum interior mutability").

### XML Namespaces

Support for XML Namespaces is in three parts:

1. The XML parser uses a flat mapping (NamespaceMap) of prefix to URI while it constructs the set of in-scope namespaces.
2. The smite tree implementation has a Namespace type node that represents an XML Namespace declaration.
3. Transformations can store the flat mapping (NamespaceMap) of prefix to URI so it can use that, if required, during the transformation.

There is a convenience routine that builds the flat mapping, i.e. a NamespaceMap, from the Namespace node declarations.

NamespaceNode objects are Rc-shared because they are often used but rarely change.

## Security

XML documents and XSLT stylesheets are able to access external resources and so pose a security risk. χrust provides [a mechanism](https://docs.rs/xrust/latest/xrust/security/index.html) to be able to restrict access to certain resources. χrust is "secure by default" - resources default to the most secure restriction, which is often to not allow access at all, and an application must override the default in order to permit the resource to be used.

χrust allows a module to define a _security feature_. A security feature determines whether access to a resource is permitted, and if so it may also constrain the use of the resource to some limit. For example, if the resource is an external entity then one security feature will control whether the entity can be access at all and a second security feature may limit the size of the entity.

An application may create a _security policy_ that includes a number of features. When the module needs to access the resource, it will lookup the required feature in the current policy and evaluate it to determine whether the resource may be accessed and/or the amount that may be consumed.

## Applications

There are two applications for using χrust:

* [xrusty](https://gitlab.gnome.org/World/Rust/markup-rs/xrusty) - a command line interface
* [xrunchy](https://gitlab.gnome.org/balls/xrunchy) - use χrust in a Web page via WASM

## The Plan

1. Complete the XPath 1.0 implementation. (Done!)
2. Implement all XSLT v1.0 functionality. (Done!)
3. Improve XDM, XPath; achieve v2.0 compliance.
4. Add v2.0 features to the XSLT engine.
4. Further improve XDM, XPath; achieve v3.1 compliance.
5. Add remaining v3.0 features to the XSLT engine.

NB. We're picking the low-hanging fruit first. So major, fundamental features of the languages are being implemented to begin with. The fine detail will be added later.
Although the eventual desire is to implement all of XSLT v3.0 functionality, some more advanced features will be implemented sooner rather than later.

## Documentation

See the [XSLT module](https://docs.rs/xrust/latest/xrust/xslt/index.html) for an example of how to evaluate an XSL stylesheet.

## Examples

* [Integration](https://gitlab.gnome.org/World/Rust/markup-rs/xrust/-/blob/main/examples/ixml.rs) with [Invisible XML](https://www.w3.org/community/ixml/2021/03/19/welcome-to-ixml/).
* [An example](https://gitlab.gnome.org/World/Rust/markup-rs/xrust/-/blob/main/examples/issue-30.rs) using XPath, but not XSLT.

## Compliance

Status of [standards implementation](https://gitlab.gnome.org/World/Rust/markup-rs/xrust/-/blob/main/docs/compliance.md) for XDM, XPath, XQuery, and XSLT.

## Release Notes

| Releases      | Notes                                                                                    |
|---------------|------------------------------------------------------------------------------------------|
| Version 2.3.0 | Added tokenize XPath function. Various bug fixes. |
| Version 2.2.0 | Add security policy documents. |
| Version 2.1.0 | Introduce security mechanism. |
| Version 2.0.3 | Fix ambiguity in forward and reverse axis names. |
| Version 2.0.2 | Rationalise and simplify use of QNames in NameTest. |
| Version 2.0.1 | Bug fix for smite::namespace_iter(). |
| Version 2.0.0 | Use qualname::QName to intern Qualified Names. Update all APIs to use QNames. Improve XML Namespace support. Improve error reporting. Upgrade benchmarks. |
| Version 1.3.0 | Implement xsl:for-each-group/@group-starting-with. Improved serialisation of XML, including canonical format. Fix bug in adding attribute node to result tree. Support predicates for postfix expressions. |
| Version 1.2.5 | Fixed disable-output-escaping. |
| Version 1.2.4 | Improved performance of DTD validation. Bug fixes for ANT declaration and white space handling. |
| Version 1.2.3 | Fixed handling of '<' character in attribute value.                                      |
| Version 1.2.2 | Fixed node() kind test. Fixed union in Pattern. Fixed relative path in Pattern.          |
| Version 1.2.1 | Fixed xsl:sort elements. Fixed 'already borrowed' bug in smite.                          |
| Version 1.2   | Added ID/IDREFS support. Added XPath 2.0 functions: avg(), min(), max().                 |
| Version 1.1   | Added new_document() to Node trait. Tree documents can now be created in one step. eg. smite::RNode::new_document() |
|               | XML Namespace support has been redesigned. Node names are now Rc-shared.                 |
