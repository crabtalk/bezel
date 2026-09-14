---
title: Syntax
description: A fence tag and a source string in, `(byte range, kind)` spans out — and the one function pointer that lets an app colour a language bezel has never heard of.
---

```rust
syntax::highlight(code, "rs") // -> Option<Vec<(Range<usize>, HighlightKind)>>
```

Spans in document order, in bytes; everything outside them is plain text. A tag naming no grammar returns `None` and the block renders plain.

## Installing it

```rust
fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    syntax::highlight(code, language)
}

markdown::set_highlighter(cx, spans, syntax::lang::LANGS.iter().map(|lang| lang.name));
```

`markdown` does not depend on `syntax` — the two meet at that function pointer. Note the argument order flips: `Highlighter` takes the language first, `syntax::highlight` takes the source first, and both are `&str`, so a swap compiles and silently colours nothing.

## A language of your own

```rust
use syntax::lang::Lang;

static ZIG: Lang = Lang::new(
    "zig",
    &["zig"],
    tree_sitter_zig::LANGUAGE,
    include_str!("../queries/zig.scm"),
);

ZIG.highlight(code)
```

`Lang::new` is `const`, so it sits beside the built-in rows and reaches the same query cache and `HighlightKind` vocabulary. Take the grammar's `LanguageFn` through `syntax::tree_sitter_language` — two tree-sitters in one graph are two unrelated types with the same name, and `Lang::new` rejects the stranger.

## Features

```toml
syntax = { version = "0.0.2", default-features = false, features = ["rust"] }
```

One per language, all on by default. Seven grammars down to one is a measured 12.4s clean build down to 3.5s. With none on, the table is empty and every block paints plain.

## API

| | |
| --- | --- |
| `highlight(code, tag)` | Eight languages, each answering to its fence aliases — `rust`/`rs`, `python`/`py`, `typescript`/`ts`, `tsx`/`jsx`/`javascript`/`js`, `json`/`jsonc`, `go`/`golang`, `bash`/`sh`/`shell`/`zsh`/`console`, `toml`. |
| `HighlightKind` | Kinds become colours through `SyntaxPalette`; a capture name with no slot degrades to `Variable`. |
| `Lang::new(name, aliases, language, query)` | For another tree-sitter grammar. |
| the function pointer | For another *engine* — `Range<usize>` and `HighlightKind` are the whole vocabulary, so syntect or a regex pass is the same function with a different body. |

`syntax` is a peer crate, not part of the `bezel` facade, so an app that highlights nothing never compiles a grammar. A browser cannot run any of it — tree-sitter is C and `wasm32-unknown-unknown` has no libc — so the gallery highlights its samples in a build script and the wasm build looks the answer up.

The source is at `apps/gallery/src/patterns/syntax.rs`, and the highlighter it installs at `apps/gallery/src/highlight.rs`. Copy the file.
