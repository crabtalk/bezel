# bezel-vega-lite

[Vega-Lite](https://vega.github.io/vega-lite/) 6.5's compiler: a Vega-Lite spec compiled to the Vega spec `vegaLite.compile(spec).spec` writes, as a `bezel-vega` spec. No JavaScript engine and no GUI toolkit.

```rust
let spec: vega_lite::spec::TopLevel = serde_json::from_str(text)?;
let vega = vega_lite::compile(&spec)?;
```

## Tests

```sh
cargo nextest run -p bezel-vega-lite
```

`tests/corpus/specs/` holds Vega-Lite's example gallery at v6.5.0 and `tests/corpus/data/` the vega-datasets 3.2.1 files those specs name. `tests/reference/` pins Vega 6.4.0, Vega-Lite 6.5.0 and vega-datasets 3.2.1; after `npm install` there, `node tests/reference/reference.mjs <dir>` writes what they make of each spec. `VEGA_REFERENCE=<dir> cargo nextest run -p bezel-vega-lite --run-ignored all` compares each compiled spec with Vega-Lite's; the specs that match exactly are those in `tests/compile.txt`. `VEGA_SPEC=<name>` checks one spec and prints where it differs.
