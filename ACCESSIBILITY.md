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
| Last reviewed | 2026-11-12 |

## What it emits that people read

- **A transform's result** (`urn:xslt:transform`, `transform_xml`, `CompiledStylesheet` in
  `src/lib.rs`): HTML, XML or text, as the stylesheet's `xsl:output method` says. For the html
  method, `src/html.rs` changes one thing in xrust's serialization: an empty element HTML does not
  call void keeps its end tag (`<script src='a.js'></script>`, `<div></div>`), so a browser does
  not misnest the rest of the page.
- **Errors and refusals** (`src/lib.rs`, `src/limits.rs`, `src/subset.rs`,
  `src/parameters.rs`, `src/boolean_values.rs`, `src/numeric_values.rs`, `src/modules.rs`, `src/output_contract.rs`): a missing input names
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

Every named check has a result: Pass, Fail or Blocked, with evidence. A Pass covers only
the stated check, not overall accessibility. The Pass checks below were run on 2026-11-12 against
main commit [`e2447795db79b04366a43ccf2884df7bb3d1d91f`](https://github.com/ikigai-rs/ikigai-xslt/commit/e2447795db79b04366a43ccf2884df7bb3d1d91f),
before this documentation update. The native test command was `cargo test --workspace`, using
Cargo 1.99.1 and rustc 1.99.1 on macOS (aarch64-apple-darwin). CI repeats the native tests through
[the Rust workflow](.github/workflows/ci.yml); it does not repeat browser or assistive technology
checks. Ignored measurement tests are not evidence of a Pass.

| Check | Result | Evidence |
|---|---|---|
| HTML serialization: end tags, namespaces, escaping, inferred output method and endpoint output | Pass | `cargo test --workspace`: `tests/html_output.rs`, including `an_empty_non_void_element_keeps_its_end_tag_and_a_void_one_has_none`, `an_element_in_a_namespace_is_serialized_as_xml`, `escaping_is_unchanged`, `with_no_output_method_an_html_root_means_html` and `the_endpoint_serves_html`. Checks serialized strings, not browser parsing or page semantics. |
| Unsupported inline HTML raw text and output requests | Pass | Same command: `tests/html_output.rs::inline_script_text_that_needs_raw_serialization_is_refused` and `tests/xslt_audit.rs::{raw_html_text_and_recursion_limits_refuse_the_whole_result,unsupported_output_and_patterns_have_named_typed_refusals}`. Checks typed errors and script/style refusals, including mixed case; no browser or assistive technology. |
| Input depth, time, capacity and engine errors | Pass | Same command: non-ignored tests in `tests/xml_depth.rs`, `tests/time_budget.rs`, `tests/engine_panic.rs` and unit tests and doctests in `src/limits.rs`. Checks bounded execution and error types, input names, limits and selected caller actions; does not establish that people understand the messages. |
| Unsupported constructs and parameter refusals | Pass | Same command: `tests/xrust_subset.rs::unsupported_constructs_are_refused_by_name` and `tests/parameters.rs::{only_declared_top_level_parameters_are_accepted,malformed_parameter_declarations_are_refused}`. Checks named refusals, not usability. |
| Module refusals name hrefs and dependency chains | Pass | Same command: `tests/modules.rs`, including `missing_modules_are_not_found_and_name_the_href`, `an_inline_stylesheet_refuses_a_relative_href_and_accepts_an_absolute_one`, `cycles_name_the_whole_href_chain` and `deep_module_chains_are_refused`. Checks error text and types, not spoken presentation. |
| Browser parsing, keyboard operation and focus in generated HTML | Blocked | No browser or manual keyboard session was available or run for this review. Tests compare serialization only; caller stylesheets and host pages need evaluation. |
| Automated accessibility checker | Blocked | No axe or other automated accessibility checker was configured or run on generated output for this review. Rust tests do not evaluate WCAG page requirements. |
| Visual contrast, zoom, reflow and terminal report layout | Blocked | No rendered browser or terminal visual evaluation was run. Stylesheets control page colors and layout; example reports use space-aligned columns. |
| Screen reader | Blocked | No screen reader session was available or used on the output for this review. |
| Screen magnifier | Blocked | No magnifier session was available or used on the output for this review. |
| Speech input | Blocked | No speech-input session was available or used on the output for this review. |
| Switch access | Blocked | No switch-access session was available or used on the output for this review. |
| Braille display | Blocked | No braille display was available or used on the output for this review. |

## Known limitations

Source limitations reviewed on 2026-11-12, not by testing with people or assistive technology:

- The HTML serializer uses XML-compatible quoting and valued boolean attributes. Inline
  `script` or `style` text that needs raw serialization is refused before any document is
  returned; keep it in external files. `tests/xslt_audit.rs` checks the typed refusal for
  both element names and mixed case. No browser or assistive technology was used.
- The panic refusal is a single paragraph of several sentences.
- The examples' reports align columns with spaces (`{label:<18}`), so their meaning depends on
  position.

## Contributing

Read this file before changing any output listed above: the serialization of a result, an error's
words, or an example's report. Keep the result the stylesheet's, keep each refusal typed and
naming its input, and say what to do where the crate can know. In the pull request, give each
accessibility check a `Check | Result | Evidence` row with Pass, Fail or Blocked, the test or
command and tool version, or why it could not run. Keep unrun checks as Blocked, including each assistive technology;
keep historical runs that CI does not repeat Blocked with their dates.

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
| 2026-11-11 | Removed internal tracking numbers from refusals and put caller actions first in time, capacity, attribute, boolean and numeric refusals. Regression tests check the text; no assistive technology testing. |
| 2026-11-11 | The unpublished audit fixes text escaping and refuses unsupported namespace/output requests and HTML raw text; recursion refusals name the stylesheet and suggest reducing recursion. Synthetic tests cover these errors; no assistive technology testing. |
| 2026-11-12 | Added scoped Pass / Fail / Blocked evidence for current main; recorded browser, visual and assistive technology checks as Blocked. |
