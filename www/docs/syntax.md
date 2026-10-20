---
title: Syntax
description: A fence tag and a source string in, `(byte range, kind)` spans out — and the one function pointer that lets an app colour a language bezel has never heard of.
prompt: Colour the Rust and TOML code fences in my markdown documents, and only compile those two grammars.
---

```rust
use markdown::AppExt as _;

fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    syntax::highlight(code, language)   // source first, language second
}

syntax::grammars::install();            // once, before any highlight
cx.set_highlighter(spans, syntax::registry::ready());
```

- `syntax::highlight` returns `None` for a tag with no grammar in the registry, and that fence paints plain. Nothing has a grammar until `grammars::install` or `registry::register` runs.
- `markdown::Highlighter` takes the language first and `syntax::highlight` takes the source first. Both are `&str`, so swapping them compiles and colours nothing.
- Build a grammar of your own against `syntax::tree_sitter_language`. A second tree-sitter in the dependency graph is a different type, and `Lang::new` rejects it.
- `syntax` is not part of the `bezel` facade, and it does not build for `wasm32-unknown-unknown`.

## Make it yours

| call | what it decides |
| --- | --- |
| features `rust`, `python`, `typescript`, `json`, `go`, `bash`, `toml` | which grammars compile in; all on by default |
| `Lang::new`, `Grammar::Native` | a language of your own, as a `static` beside the built-in ones |
| `Lang::with_injections` | regions written in another registered language |
| `registry::register`, `registry::Entry` | which fence tags and file names reach a grammar |
| `registry::of_tag`, `registry::of_path` | what a tag or a path names, painted or not |

```toml
syntax = { package = "bezel-syntax", version = "0.2", default-features = false, features = ["rust", "toml"] }
```
