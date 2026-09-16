# Test fixtures

`json.wasm` and `css.wasm` are tree-sitter grammars compiled to WebAssembly, for the tests in `tests/wasm.rs`. This crate links no grammar of its own, so without them the wasm load path has nothing to load.

`css.wasm` carries an external scanner — `tree-sitter-css` ships `scanner.c` beside `parser.c`, and a grammar built without it silently fails to match its external tokens. It is here so that case is covered.

## Rebuilding

They are built from the `parser.c` that the `tree-sitter-json` and `tree-sitter-css` crates ship, using the tree-sitter CLI:

```sh
cargo install tree-sitter-cli --version 0.27.0 --features wasm
tree-sitter build --wasm -o json.wasm ~/.cargo/registry/src/*/tree-sitter-json-0.24.8
tree-sitter build --wasm -o css.wasm  ~/.cargo/registry/src/*/tree-sitter-css-0.25.0
```

`wasm` is not one of the CLI's default features. The first build downloads a wasi-sdk clang and binaryen to a cache directory — around 180 MB — so there is no system toolchain to install and nothing to configure.

The output is reproducible: both files here were verified byte-identical to a fresh CLI build at the versions above.
