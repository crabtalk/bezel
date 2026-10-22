# bezel-vega

[Vega](https://vega.github.io/vega/) 6.4's runtime: a Vega spec run to the scenegraph Vega draws from it, and the view's size, origin and padding. No JavaScript engine and no GUI toolkit; text widths are Vega's own estimate.

```rust
let spec: vega::spec::Spec = serde_json::from_str(text)?;
let view = vega::View::new(spec, &|url| load(url))?;
let drawing = view.drawing()?;
```

`expr` evaluates Vega expressions with JavaScript's semantics, on `value`'s JavaScript values.

## Tests

```sh
cargo nextest run -p bezel-vega
```

`VEGA_REFERENCE=<dir> cargo nextest run -p bezel-vega --run-ignored all scenegraph` runs each Vega spec Vega-Lite compiles the gallery to and compares the scenegraph and view placement with Vega's, numbers to a hundredth; the specs that match are exactly those in `tests/scenegraph.txt`. `VEGA_REFERENCE=<dir> cargo nextest run -p bezel-vega --run-ignored all the_data` runs each spec's data sets, signals and view size, and compares each top-level data set's tuples with `view.data(name)`, numbers exactly; the specs that match are exactly those in `tests/data.txt`. `VEGA_REFERENCE=<dir> cargo nextest run -p bezel-vega --run-ignored all the_scales` runs the same with the spec's scales, and compares each top-level scale's type, domain, range, bandwidth and what it maps samples of its domain to, numbers exactly; the specs that match are exactly those in `tests/scales.txt`. Each check reads times in UTC, as the reference does, and `--no-capture` prints where each other spec differs. `<dir>` is written by `node crates/vega-lite/tests/reference/reference.mjs <dir>`, after `npm install` there. `VEGA_SPEC=<name>` checks one spec and prints where it differs.
