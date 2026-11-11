# Synthetic XSLT 1.0 / XPath 1.0 audit, 2026-11-11

Scope: ikigai-xslt 0.9.0 with the carried ikigai-xrust 2.3.0-ikigai.2 fork,
starting at main commit `1f176d9405768dcfaef6e009cfbf51041b41af50`. No version bump,
publication, upstream submission or external conformance-suite download is involved.
All new fixtures and tests are synthetic, authored here. Tests never fetch data.

The reference semantics are [XSLT 1.0](https://www.w3.org/TR/1999/REC-xslt-19991116)
(§5.2, §5.5, §5.8, §7.2, §7.6, §16) and
[XPath 1.0](https://www.w3.org/TR/1999/REC-xpath-19991116/)
(§3.4–3.5 and §4.2). This is a bounded audit, not a conformance claim.

## Reproduction and repairs

Before changing production code, the initial `cargo test --test unled_audit --
--nocapture` run had 2 passing and 12 failing tests. The assertions compared real
transform output with expected output or a typed, named refusal. The boolean
minimization assertion was subsequently classified as an advisory syntax difference,
not a behavioral defect. Later tests isolated function conversions, raw-text HTML,
and the child-pattern node-type error. The relevant regressions are retained below.

| Reproduced behavior | Resolution | Regression |
| --- | --- | --- |
| `<xsl:text>&lt;&amp;</xsl:text>` inside `out` emitted `<out>&amp;lt;&amp;amp;</out>`, double escaping characters | Store text characters until serialization; preserve explicit disable-output-escaping | `text_is_escaped_once`, `xslt_text_escapes_only_during_serialization` |
| Two `match="item"` templates with equal priority emitted `first`, with no error, rather than the last declaration | Record document order and compare equal priorities as equal | `template_priorities_and_ties`, `xslt_equal_priority_uses_last_declaration` |
| Applying templates to an element in mode m with no explicit element rule errored instead of recursing in mode m; comments/PIs also errored | Supply built-in rules when no declared rule applies, preserving mode | `built_in_templates_keep_the_mode`, `xslt_builtin_templates_cover_modes_and_node_types` |
| The built-in `child::*` pattern also matched an attribute, losing its value: `ATB` became `TB` | Restrict child name tests to element nodes, and exclude attribute/namespace nodes from child kind tests | `built_in_node_types`, `xslt_child_name_patterns_do_not_match_attributes` |
| For two items `one`, `two`, value-of, string(), string-length(), concat(item, '!') returned `onetwo`, `onetwo`, `6`, `onetwo!` | Adapt XPath 1.0 first-item string conversion before compilation | `multiple_nodes_become_the_first_string`, `string_functions_convert_first_node_and_empty_sets` |
| `xsl:namespace-alias` silently left a literal name in the stylesheet namespace | Named `InvalidArgument` refusal; write result names directly | `namespace_alias_is_applied_or_refused` |
| Explicit UTF-16 encoding was ignored and UTF-8 returned; explicit omit-xml-declaration=no still emitted no declaration | Named refusals; supported output remains UTF-8 XML fragments without a declaration | `unsupported_encoding_is_refused`, `xml_declaration_is_honored_or_refused` |
| HTML script text `a < b && c = 'd'` became entity references, changing executable code | Refuse script/style text requiring XML escaping before returning a result; use external files | `inline_html_code_is_raw_or_refused`, `inline_script_text_requiring_escaping_is_refused` |
| Evaluation depth exhaustion lost its engine error kind and became an endpoint-error string | Preserve it as `InvalidArgument` naming stylesheet, suggesting less recursion; never return the partial result | `recursion_is_a_typed_refusal`, `refusal_types_survive_the_endpoint` |

Each engine fix has a separate commit and a ready-to-review upstream patch:
[text escaping](upstream/xrust-text-escaping.md),
[template ties](upstream/xrust-template-ties.md),
[built-in rules](upstream/xrust-builtin-templates.md),
[child pattern node types](upstream/xrust-pattern-node-types.md).
The XPath 1.0 source adaptation deliberately stays in the wrapper: changing xrust's
sequence string semantics globally would also change its newer XPath behavior.

The tested escaped-brace AVT (`{{literal}}`), a leading `//` match pattern, and a targeted
`processing-instruction('target')` pattern already errored, rather than returning an
incorrect document. Their errors were untyped compiler/module messages. They now
have documented `InvalidArgument` refusals naming the construct. This is error
classification, not a claim of previously silent wrong output for those cases.

## The seven previously skipped areas

| Area | Public behavior after this audit | Evidence |
| --- | --- | --- |
| Pattern predicates and template priority | Pattern predicates remain named refusals; default name/wildcard/path/namespace priorities and explicit priorities work; ties now choose the last declaration | `skipped_areas`, `template_priorities_and_ties`, `priorities_include_namespace_wildcards_and_pi_targets`, existing `xrust_subset` and `modules` suites |
| position(), last(), [last()] | Named refusals away from the root / for last() predicates, with the existing root-context exceptions | `skipped_areas`; `refused_position_and_last_away_from_the_root`, other `xrust_subset` tests |
| Leading // | XPath expressions away from the root remain named refusals; leading // in match patterns is now a named refusal too | `skipped_areas`, existing `refused_a_leading_double_slash_away_from_the_root` |
| value-of with several nodes | Correct first-node string value; string(), string-length() and the XPath 1.0 string-taking functions likewise use first-node conversion | `multiple_nodes_become_the_first_string`, `string_values`; copy-of still copies all nodes |
| Numeric string comparison | Correct: `'10' < '2'` is false, and node-set comparisons consider all numeric pairs | `skipped_areas`, existing `numeric_conversion` matrix |
| Unary signs | Unary minus, including nested signs and signed zero, is adapted correctly; unary plus is an accepted extension rather than XPath 1.0 syntax | `skipped_areas`, existing `numeric_conversion` matrix |
| Large integer literals | Correct double conversion through the existing adapter | `skipped_areas`, existing `boolean_numeric_and_large_literal_upstream_causes` and `numeric_conversion` |

Except for the repairs explicitly listed above, the behavior in this table already
held at the starting commit: the upstream skipped numeric/sign/literal changes
were already adapted in the public wrapper.

## Other requested checks and limits

- Import precedence, include order, output-property inheritance, named bindings and
  module refusals retain the existing `tests/modules.rs` coverage. The union namespace
  and recursive parameter regressions from the earlier fixes remain in
  `tests/union_namespaces.rs` and `tests/recursive_parameters.rs`.
- New cases exercise built-in rules for root/element descent, attributes, text,
  comments and processing instructions, including an undeclared mode. Namespace
  nodes have an empty built-in fallback. This is not an exhaustive namespace-axis audit.
- Excluded unused namespaces stay absent; an excluded prefix still appears when a
  result name requires it. The earlier union/import rebinding regressions remain.
  This does not claim complete namespace-node propagation conformance.
- Existing HTML tests pin void elements (`<br/>`), explicit closing tags for empty
  non-void elements (`script`, `textarea`, `div`), and XML serialization for foreign
  namespaces. HTML boolean attributes remain quoted and unminimized:
  `checked='checked'`. This preserves HTML boolean semantics; §16.2's minimization
  recommendation is a SHOULD. No claim of byte-for-byte canonical HTML is made.
- Text output preserves characters; XML text is escaped once, including explicit
  disable-output-escaping=no. Explicit disabling on value-of remains supported;
  the engine regression additionally covers disabling on xsl:text. XML without an
  explicit declaration remains the existing fragment serialization. Requests for
  a declaration or another encoding are now refused instead of silently ignored.
- Repeated transforms through one compiled stylesheet are byte-identical in the
  new namespace case and existing HTML/generate-id tests. The full source is the
  compilation cache key; no timing or random values are added to output.
- `tests/time_budget.rs` exercises typed Timeout/Unavailable results and caps
  overdue work; `tests/xml_depth.rs` covers input-depth refusals. The recursion
  case starts an `out` containing `partial` and then recurses forever: only the
  typed error reaches the caller, including through the endpoint.

## About-page evidence

Before production changes, the existing `foaf-last-segment.xsl` and RDF fixture
rendered to the 749-byte `tests/fixtures/foaf-last-segment.expected.html` baseline.
`about_shape_is_byte_identical` compares the complete output, including all six
sections, against those exact bytes. The existing FOAF union fixture also retains
its golden output. These fixtures explicitly model the production parameter/union
shapes; they are not the deployed `services/edge/config/foaf.xsl` file.

**Live-page limitation:** the deployed stylesheet and input are not present in this
checkout. The real live About page was not rendered or compared. Output changes
in this PR correct special-character xsl:text and multi-node conversion, and
refuse previously corrupted HTML inline code. If the deployed stylesheet uses
those cases, its output can change or be refused. The fixture comparison proves
only the recorded FOAF shapes unchanged; deployment-level byte identity requires
the actual stylesheet/input. No other checkout was accessed.

## Validation

Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo test --workspace`, and the workspace library clippy gate with
`--target wasm32-unknown-unknown --features module -- -D warnings`.
The pull request records the completed local and CI results. No browser,
accessibility scanner or assistive technology was used.
