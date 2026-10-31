# Source cost on RSS

The compile cache removes a fixed stylesheet cost. It does not cache source trees or
results. Large RSS inputs can therefore exceed the native default **5 s transform-worker
budget**, even with a warm stylesheet. There is no byte-size cutoff that guarantees a
feed will finish: input shape and template work matter as well as bytes. Validation and
input resolution happen before the worker's timer; this is not a total ingestion deadline.

For arithmetic-heavy workloads, see the separate [numeric adapter investigation and
chart-sized operand-cost comparison](numeric-cost.md). It measures numeric wrappers
independently; RSS throughput is not a proxy for their cost.

## Reproduction

Measured 2026-10-30 on an Intel Core i7-11800H, Linux x86_64, rustc 1.99.0,
`cargo --release`, ikigai-xslt 0.5.0, xrust 2.3.0, ikigai-core 0.2.8.
These are synthetic, offline inputs, not the 112-feed census reported against
ikigai-cli 0.2.4 in ledger #2522. The original feed files and `items.xsl` were not
supplied, so these measurements do not independently verify that census's exact
350 KB/s or its three particular timeouts.

Before changing the benchmark, the existing `stylesheet-cost` example with a generated
1,024,881-byte, 1,000-item RSS source took 258 ms (median of five) with a reused stylesheet,
versus 11.4 ms to compile the 527-byte stylesheet. A 1,132,881-byte, 5,000-item source
started at 2.4 s per reused transform. This reproduced the source-dominance claim and
motivated splitting source cost. The formatted fixture committed here is 624 bytes;
it selects `/rss/channel/item`, conditionally copies title and link, and calls a date
template per item. It ignores `content:encoded`.

Reproduce the stage measurements without downloading feeds:

```sh
cargo run --release --example stylesheet-cost -- --rss 100 10000
cargo run --release --example stylesheet-cost -- --rss 1000 850
cargo run --release --example stylesheet-cost -- --rss 5000 50
cargo run --release --example stylesheet-cost -- --rss 10000 0
```

The two numbers independently control item count and the number of `x` bytes in each
item's CDATA HTML body. The first three sources are each about 1 MB. A body of zero
still includes the CDATA `<p></p>` wrapper. A real stylesheet and feed can be measured too:

```sh
cargo run --release --example stylesheet-cost -- items.xsl feed.xml
```

This is a trusted-input measurement tool, not an ingestion service. Direct xrust and
`CompiledStylesheet` calls have no deadline; the `transform_xml` row uses
`transform_xml_within` with **one hour**, so a slow measurement can finish. All work runs
on stacks sized by `limits::on_xslt_stack`. File I/O is excluded.

## Where time goes

Times below are medians of five runs in milliseconds. `compile (crate)` includes its
validation and teardown. `reuse compiled` is `CompiledStylesheet::transform`, including
source validation, source parse, evaluation, serialization and teardown. The public API
row also includes dispatch and input copies; its first run pays the stylesheet compile,
and its remaining runs reuse it. No input fetches are involved.

| Items | CDATA body bytes/item | Source bytes | Compile (crate) ms | Reuse compiled ms | Public API, 1 h budget, ms |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 10,000 | 1,017,381 | 14.70 | 26.15 | 26.74 |
| 1,000 | 850 | 1,024,881 | 14.99 | 299.06 | 311.11 |
| 5,000 | 50 | 1,132,881 | 14.76 | 2,668.73 | 2,661.36 |
| 10,000 | 0 | 1,767,881 | 15.08 | 8,197.43 | 8,215.47 |

| Items | Source guard ms | Source parse ms | Evaluation ms | XML serialization ms |
| ---: | ---: | ---: | ---: | ---: |
| 100 | 0.47 | 21.66 | 3.63 | 0.27 |
| 1,000 | 0.52 | 204.66 | 88.57 | 2.72 |
| 5,000 | 0.68 | 1,080.79 | 1,549.38 | 15.69 |
| 10,000 | 1.19 | 2,281.41 | 5,787.12 | 35.94 |

[Full five-run output](rss-cost-results.txt) records the variation, including cold runs.
A separate call to `transform_xml` with the same 10,000-item source returned
`timeout: this XSLT transform ran past its time budget of 5000 ms` after 5.003 s,
with no result. The probe waited for the overdue worker to drain before exiting.

The source stages use **stock xrust `RNode`**, while the complete crate timings use its
`IdNode` wrapper. Source validation is the crate's real guard. Parsing returns a complete
source tree; evaluation clones the compiled context, installs that tree and a fresh result
document, builds a static context and evaluates it. Serialization uses the already
evaluated result's XML serializer. Each stage excludes its supplied setup and the drop
of its returned value and supplied state; evaluation still includes its local context
setup/teardown. Thus stage columns explain the cost but are not an exact additive
accounting of the crate's full call. HTML postprocessing is not measured by the XML stage.

For the 1,000-item, roughly 1 MB input, parsing (~205 ms) and evaluation (~89 ms) dwarf
stylesheet compilation (~15 ms); source validation (~0.5 ms) and serialization (~2.7 ms)
are small. The similarly sized 100-item source is much cheaper. From 1,000 to 5,000 items,
parse time grows about fivefold but evaluation grows about seventeenfold. This is evidence
of super-linear evaluation for this fixture, not proof of a particular internal hot
function. CDATA body bytes alone do not explain the cost. Real feeds with different text,
entities, element counts, or a stylesheet that processes the bodies can behave differently.

## What a reader can plan around

This change documents a bound rather than changing the engine or increasing the default:

- The native endpoint's worker budget is fixed at `limits::DEFAULT_TIME_BUDGET` (5 s).
  A timeout returns an error and no partial result. Fetching and validation need separate
  host limits.
- A native host using the string API can call `transform_xml_within` with an explicit
  larger budget, chosen using its own feeds, stylesheet and hardware. A compiled
  stylesheet alone supplies no deadline. On wasm neither API enforces a deadline.
- Treat `Timeout` as a per-feed failure and retain the previous successful feed result
  if appropriate for the reader. Abandoned work keeps running; immediate retries can
  increase load. New calls can get transient `Unavailable` until overdue workers drain.
- Where semantics permit, prepare bounded item batches before transforming. This moves
  full-feed parsing and limits into the host; it cannot make an arbitrary stylesheet
  that depends on the whole document equivalent to a batched one.

The small-source compile-cache benefit remains real. It should not be used to estimate
large-source throughput, and these sample times are measurements rather than a service
level guarantee. Runtime behavior and the default budget are unchanged.
