# bezel-chart

Charts for gpui: a subset of [Vega-Lite](https://vega.github.io/vega-lite/) over columnar data.

## Vega-Lite

The `vega-lite` feature adds `vega_lite::Spec`, which reads a Vega-Lite spec into a `Chart`, and `vega_lite::import` for a spec that names no url. bezel does no I/O: `Spec::urls` lists the `data.url`s a spec names, and `Spec::chart` takes a loader that answers each with its text. An image mark's url is the host's to answer as well, with the function `view::set_image_loader` installs.

## Scenegraphs

`chart::scene` paints a `vega::Drawing`, the scenegraph `bezel-vega` runs a Vega spec to, with gpui: groups, rects, rules and text, text upright.

## Tests

```sh
cargo nextest run -p bezel-chart --features vega-lite
```

`tests/transform/` holds specs with the data Vega computes for each, written by `bezel-vega-lite`'s `tests/reference/fixtures.mjs`.
