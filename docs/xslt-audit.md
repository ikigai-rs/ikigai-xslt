# XSLT 1.0 / XPath 1.0 audit, 2026-11-11

Synthetic cases in `tests/xslt_audit.rs`, the existing subset/module tests, and
`crates/xrust/tests/smite-xslt.rs`. These tests do not fetch anything. No third-party
conformance fixtures were vendored. This is a measured subset, not a conformance claim.

Normative references: [XSLT 1.0](https://www.w3.org/TR/1999/REC-xslt-19991116)
and [XPath 1.0](https://www.w3.org/TR/1999/REC-xpath-19991116/).

## Previously skipped areas

| Area | Before this audit | After this audit / evidence |
| --- | --- | --- |
| Match predicates | Named, typed refusal before compilation | Remains refused; `xrust_subset::refused_a_predicate_in_a_match_pattern`, including the engine's wrong answer |
| Default/computed and explicit template priorities | Module assembly already computes priorities, splits union alternatives and encodes import precedence | Passing priority and import tests in `modules`, `union_namespaces`, and `xslt_audit`; predicates remain refused |
| `position()` / `last()` | Existing root-context exceptions and typed refusals; a template with both `name` and `match='/'` bypassed them and produced `11` when called from a two-node for-each | Named root rules are conservatively treated as non-root; `a_named_root_rule_cannot_bypass_context_sensitive_refusals` failed with an unexpected successful `11` before the guard change |
| Leading `//` in patterns | Engine compilation error, not a silent wrong answer | Named `InvalidArgument` for template match patterns; use a name pattern to match at any depth |
| Leading `//` in expressions | Correct at the root; typed refusal elsewhere | Existing subset regressions retained |
| Value-of over multiple nodes | **Wrong:** `doc/v` on `<doc><v>2</v><v>10</v></doc>` produced `210` | First node gives `2`; empty selection gives empty text; scalar booleans/numbers still work |
| Other first-node string conversions | **Wrong:** `string(doc/v)` gave `210`; `string-length(doc/v)` measured all nodes; `{doc/v}` gave `210` | Core string-function arguments and AVT expressions use the first node too; public-path regressions plus the existing raw-engine comparison |
| Numeric string relational comparison | Adapter already gave numeric XPath 1.0 answers (`'10' < '2'` is false) | Retested, including nonnumeric text, boolean operands and existential node-pair comparisons |
| Unary signs | Adapter already answered signs | Retested negative and repeated unary signs; existing numeric suite covers negative zero |
| Large integer literals | Beyond-i64 literals were adapted, but adjacent representable engine integers above 2^53 compared as distinct: `9007199254740992 = 9007199254740993` gave false | Literals at or above 2^53 compile as doubles; equality gives true; number() conversion also produces a double |

## Additional reproductions and repairs

Each engine repair has its own commit and [upstream patch draft](upstream/README.md).
Nothing has been submitted upstream or published.

| Reproduction | Wrong or failed result before repair | Resolution / regression |
| --- | --- | --- |
| `xsl:text` containing `&lt;&amp;` | `&amp;lt;&amp;amp;` (escaped twice) | Create a text node and escape once; `text_escaping_and_disable_output_escaping_work_in_each_method` |
| `xsl:text disable-output-escaping='yes'` containing escaped markup | Escaped markup rather than requested raw markup | Carry the output hint on the text node; same test covers yes/no, XML/HTML/text and the endpoint |
| `match='v'` competing with `match='@*'` on attribute `v` | Element-name pattern won on an attribute | Child-axis NameTest requires an element; KindTest continues matching other node types |
| Built-in attribute rule | Lost the attribute value | Construct text from the attribute value |
| Built-in comment/PI rules | No matching template error | Return the empty sequence |
| Built-in recursion in mode `m` | No matching template error; no recursive mode fallback | Built-ins exist in every mode and retain that mode through recursion |
| `a='{{x}}'` and quoted braces inside an AVT expression | Compilation errors; closing escapes alone were not collapsed | One bounded scan honors doubled braces, XPath quotes and complete-expression parsing |
| `doc/v[2] \| doc/v[1]` | Branch order `10,2` instead of document order `2,10` | Sort and deduplicate union nodes, using existing navigation's document-order/identity APIs |
| Standalone `xsl:output method='xml'`, followed by `method='text'` | XML markup, despite the later method declaration | Merge outputs without requiring an import; metadata inspection uses the same last-declaration recovery |
| `xsl:namespace-alias` | Literal result namespace unchanged | Typed refusal naming the instruction; no partial implementation |
| Explicit XML declaration request | Declaration request silently ignored | Typed refusal for explicit `omit-xml-declaration='no'`; UTF-8 and omission remain supported |
| Non-UTF-8 encoding request | Serializer only returns UTF-8 | Typed refusal naming encoding; tested with UTF-16. Unsupported nonmandatory encodings could legally fall back to UTF-8, so the initial US-ASCII probe is not counted as a standards defect |
| Inline HTML script/style text requiring escaping | Broken executable/style text, documented previously as a limitation | Typed refusal after evaluating the result tree, before serialization; covers dynamic text and mixed-case element names |

## Output, namespaces and bounds

- Existing HTML tests retain end tags for empty non-void elements, including external
  scripts. Void elements retain `<br/>`; boolean attributes retain values, e.g.
  `<input checked='checked'/>`. These are intentional XML-compatible HTML spellings
  with the intended browser behavior, not a claim to implement every recommendation
  of XSLT 1.0 section 16.2. `html_boolean_attributes_retain_their_documented_values`
  pins this choice. No browser validation was performed.
- XML explicitly omitting its declaration and requesting UTF-8 is tested, including
  non-ASCII text. By default this crate also omits declarations. XSLT section 16
  allows processors to limit serialization support; these explicit refusals make the
  supported choices visible. No UTF-16 encoder was added.
- An unused excluded prefix does not appear in the result. Namespaced attributes and
  elements still require their namespace declarations. Existing `union_namespaces`,
  `recursive_parameters`, and subset identity regressions remain gates, including
  imported prefix rebindings and the FOAF six-way union.
- Repeated compiled runs compare exact bytes. `time_budget` exercises the default
  five-second deadline, overdue capacity and real transforms; `xml_depth` checks
  input/parser bounds and engine recursion. A recursion-limit error now stays a typed
  `InvalidArgument` at the endpoint, names the stylesheet, suggests reducing recursion,
  and never delivers the prefix generated before the error. The audit tests check that
  behavior with an unconditional named-template recursion. First-node string adaptation
  adds two bracket levels; generated expressions still obey MAX_EXPR_DEPTH, so input
  near that bound can receive a typed refusal. The depth suite tests both outcomes.
  The direct, `!Send` CompiledStylesheet API runs on its caller's thread and has no
  wall-clock deadline; use transform_xml/the endpoint for the default deadline.
  Wasm also has no wall-clock deadline, as documented in the README.

## About page compatibility and limits of this audit

`foaf-last-segment.expected.html` was captured from the existing fixture before any
engine or adapter change. `foaf_fixture_is_byte_identical_to_the_pre_audit_baseline`
compares its complete output, byte for byte (SHA-256
`b36b64ee41f48783fff6443438908be56ad3e359702cc2136ece3c0294f9fb8d`).
The existing six-way FOAF union fixture
also keeps its complete expected output.

The repository contains FOAF-shaped synthetic fixtures, not the full live
`services/edge/config/foaf.xsl`. Attempting the corresponding GitHub contents API path
in `ikigai-rs/ikigai-gonk` returned HTTP 404 with the worker's available credentials.
No other checkout was read. **The exact live About page is therefore unverified**;
these repairs can change its output if it exercises one of the corrected behaviors,
or can refuse it if it uses an unsupported construct. This is a limitation of the
available evidence, not a claim that the live page is unchanged.
