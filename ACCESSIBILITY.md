# Accessibility

What `ikigai-xslt` emits that people read (the result of a caller's stylesheet, HTML included, and
its own error and refusal messages), the rules that output follows, how it is tested, and how to
report a barrier. It is not a conformance claim. Adapted from the
[ACCESSIBILITY.md template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA for any HTML or SVG a stylesheet produces through it; the markup is the stylesheet's, and this repository's own output is text |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/ikigai-rs/ikigai-xslt/issues/new?labels=accessibility) with the `accessibility` label; anything security-sensitive through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-xslt/security/advisories/new) |
| Last reviewed | 2026-10-31 |

## What it emits that people read

- **A transform's result** (`urn:xslt:transform`, `transform_xml`, `CompiledStylesheet` in
  `src/lib.rs`): HTML, XML or text, as the stylesheet's `xsl:output method` says. For the html
  method, `src/html.rs` changes one thing in xrust's serialization: an empty element HTML does not
  call void keeps its end tag (`<script src='a.js'></script>`, `<div></div>`), so a browser does
  not misnest the rest of the page.
- **Errors and refusals** (`src/lib.rs`, `src/limits.rs`, `src/subset.rs`,
  `src/parameters.rs`, `src/boolean_values.rs`, `src/numeric_values.rs`): a missing input names
  it (`src` or `stylesheet`); a bound past its limit names the input, the depth found, the limit
  and why (`the document nests 70 elements deep, deeper than 64 …`); an unsupported construct is
  refused by name; a transform past its time budget, a refusal while too many transforms are
  overdue, and an engine panic are each a paragraph that says what happened (and, for the first
  two, what to do).
- **The examples** (`examples/`): timing and cost reports printed to the terminal as plain lines.

Not this repository's: the markup a stylesheet writes, and with it the page's language,
headings, landmarks, text alternatives, labels and colors. A stylesheet's author owns those, and a
host that serves the result owns the page around it.

## The rules that output follows

- **The result is the stylesheet's.** Apart from the end tags above, nothing is added to or
  removed from it: no `lang`, no style, no script, and escaping is xrust's, unchanged.
- **Errors are typed and name their input.** A refusal of the caller's input is an
  `InvalidArgument` naming `src`, `content` or `stylesheet`; a missing input is a
  `MissingArgument` naming it; time and capacity refusals are `Timeout` and `Unavailable`.
- **A refusal says what to do** where this crate can know: make the input or stylesheet smaller,
  retry shortly, use `xsl:choose` with explicit text, keep scripts in their own files.
- **No color** is written by the library or the examples.

## How it is tested

- `tests/html_output.rs`: `an_empty_non_void_element_keeps_its_end_tag_and_a_void_one_has_none`,
  `an_element_in_a_namespace_is_serialized_as_xml`, `escaping_is_unchanged`,
  `with_no_output_method_an_html_root_means_html`, `the_endpoint_serves_html` and
  `inline_script_text_is_still_escaped` pin the html method's serialization.
- `tests/xml_depth.rs`, `tests/time_budget.rs`
  (`a_transform_is_answered_within_its_budget_and_overdue_work_is_capped`),
  `tests/engine_panic.rs` (`copying_the_document_node_is_an_error_not_a_panic` and the rest),
  `tests/xrust_subset.rs` (`unsupported_constructs_are_refused_by_name`) and
  `tests/parameters.rs` (`only_declared_top_level_parameters_are_accepted`,
  `malformed_parameter_declarations_are_refused`) pin which input is named and that a refusal is
  typed; the doctests in `src/limits.rs` pin that a depth refusal names its input and its limit.
- **No automated accessibility checker, browser or assistive technology has been used on this
  output.**

## Known limitations

Found by reading the source on 2026-10-31, not by testing with people or assistive technology:

- `script` and `style` text in an html-method result is still escaped (`'` becomes `&apos;`), so
  an inline script or style with a quote or a `<` in it breaks; keep them in their own files. This
  is a deliberate choice, recorded in `src/html.rs` and pinned by a test.
- Several refusals end in a project-internal tracking number in parentheses (the time-budget and
  overdue-capacity refusals in `src/limits.rs`, the attribute refusals there, the boolean and
  numeric refusals), which a reader outside the project cannot look up.
- The time-budget, overdue-capacity and panic refusals are single paragraphs of several sentences,
  and what to do comes last; the attribute refusals put an XSLT workaround inline in the same
  sentence.
- The examples' reports align columns with spaces (`{label:<18}`), so their meaning depends on
  position.

## Contributing

Read this file before changing any output listed above: the serialization of a result, an error's
words, or an example's report. Keep the result the stylesheet's, keep each refusal typed and
naming its input, and say what to do where the crate can know. Say in the pull request what was
tested, with what, and what was not.

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-xslt/issues/new?labels=accessibility) with the
`accessibility` label (the link sets it when GitHub lets you add labels; otherwise a maintainer
adds it): what you were doing, what happened, and your browser or terminal and any assistive
technology, if you are comfortable sharing them. A barrier in a page a stylesheet produced is
usually the stylesheet's; report it here when this crate's serialization causes it. Anything
security-sensitive goes through
[private vulnerability reporting](https://github.com/ikigai-rs/ikigai-xslt/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.

## Change history

| Date | Change |
|---|---|
| 2026-10-31 | First version. |
