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

`VEGA_REFERENCE=<dir> cargo nextest run -p bezel-vega --run-ignored all scenegraph` runs each Vega spec Vega-Lite compiles the gallery to and compares the scenegraph and view placement with Vega's, numbers to a hundredth; the specs that match are exactly those in `tests/scenegraph.txt`. `<dir>` is written by `node crates/vega-lite/tests/reference/reference.mjs <dir>`, after `npm install` there. `VEGA_SPEC=<name>` checks one spec and prints where it differs.
