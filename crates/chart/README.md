# bezel-chart

Charts for gpui: a subset of [Vega-Lite](https://vega.github.io/vega-lite/) over columnar data.

## Vega-Lite

The `vega-lite` feature adds `vega_lite::Spec`, which reads a Vega-Lite spec into a `Chart`, and `vega_lite::import` for a spec that names no url. bezel does no I/O: `Spec::urls` lists the `data.url`s a spec names, and `Spec::chart` takes a loader that answers each with its text. Of the 633 specs in Vega-Lite 6.5.0's example gallery, 65 import.

## Tests

```sh
cargo nextest run -p bezel-chart --features vega-lite
```

`tests/vega_lite/specs/` holds Vega-Lite's example gallery at v6.5.0, `tests/vega_lite/data/` the vega-datasets 3.2.1 files those specs name, and `tests/vega_lite/pass.txt` the specs that import; the test fails when that set changes either way. `node tests/vega/render.mjs <dir>` writes what Vega 6.4.0 and Vega-Lite 6.5.0 draw for each spec to `<dir>`, after `npm install` in `tests/vega/`.
