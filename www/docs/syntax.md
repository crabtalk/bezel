---
title: Syntax
description: A fence tag and a source string in, `(byte range, kind)` spans out — and the one function pointer that lets an app colour a language bezel has never heard of.
---

Nothing on this page is library code. It wraps a sample in a fence and hands the result to `markdown::render`. The call underneath is the crate's whole surface:

```rust
syntax::highlight(code, "rs") // -> Option<Vec<(Range<usize>, HighlightKind)>>
```

Spans in document order, in bytes, and everything outside them is plain text. No colour and no rendering here — kinds become colours through `SyntaxPalette`, and a capture name with no slot in the bezel vocabulary degrades to `Variable`, which paints as body text. A tag naming no grammar returns `None` and the block renders plain. There is no injection machinery either: the fence already names the grammar, so a block is one parse with one query.

Eight languages ship, each answering to its fence aliases — `rust`/`rs`, `python`/`py`, `typescript`/`ts`, `tsx`/`jsx`/`javascript`/`js`, `json`/`jsonc`, `go`/`golang`, `bash`/`sh`/`shell`/`zsh`/`console`, `toml`. JavaScript rides the TSX grammar, because TSX parses JS and a second grammar would buy only the `<`-ambiguity cases a highlighted sample does not hinge on.

**One feature per language, all on by default.** A grammar is a C compile, so an app that only ever shows Rust should only ever build one:

```toml
syntax = { version = "0.0.2", default-features = false, features = ["rust"] }
```

That is seven grammars down to one, and a measured 12.4s of clean build down to 3.5s. The `typescript` feature carries both the TypeScript and TSX rows, since they are one grammar crate. The language table is a slice and not a fixed-size array precisely so its length can follow the features; with none of them on it is empty, every tag resolves to `None`, and every block paints plain.

**`markdown` names no highlighter.** It does not depend on `syntax` at all — the two meet at one function pointer, installed at boot like the theme palette:

```rust
fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    syntax::highlight(code, language)
}

markdown::set_highlighter(cx, spans);
```

Note the argument order flips: `Highlighter` takes the language first, `syntax::highlight` takes the source first. Both are `&str`, so a swap compiles and silently colours nothing.

**`syntax` is a peer crate, not part of the `bezel` facade.** You name it yourself, and an app that highlights nothing never compiles a grammar — the facade carried it once, which cost every consumer seven C grammar builds and made `bezel` unbuildable for `wasm32-unknown-unknown` outright.

**A language the table does not carry is a `static` of your own.** `Lang::new` is `const`, so it sits beside the built-in rows and reaches the same `highlight` method — the query cache, the capture-name filter and the `HighlightKind` vocabulary all come with it, and none of it has to be rebuilt:

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

Take the grammar's `LanguageFn` through `syntax::tree_sitter_language` rather than declaring your own tree-sitter. Two versions in one graph are two unrelated types with the same name, and `Lang::new` will reject the stranger — the same hazard `bezel::gpui` exists to prevent, one layer down.

That extension point and the seam above it are different tools. `Lang` is for another tree-sitter grammar; the function pointer is for another *engine*, and nothing about it is tree-sitter at all — `Range<usize>` and `HighlightKind` are the entire vocabulary:

```rust
fn spans(language: &str, code: &str) -> Option<Vec<(Range<usize>, HighlightKind)>> {
    match language {
        "zig" => ZIG.highlight(code),
        _ => syntax::highlight(code, language),
    }
}
```

Swapping the engine wholesale — syntect, a regex pass, nothing at all — is the same function with a different body. This is why the grammar table stays private to `syntax`: opening it would put tree-sitter's own types in the public API, and an app carrying its own tree-sitter would then have two.

**A browser cannot run any of it.** tree-sitter is C, and `wasm32-unknown-unknown` has no libc to compile it against, so a web build would carry a dependency it can never link — which is the reason `markdown` names no highlighter in the first place. A build script runs on the host whatever the target is, so the gallery highlights its samples ahead of time and the wasm build looks the answer up by `(tag, source)`; a block the build script never saw paints plain, which is what an unknown language does anyway.

Highlighting recolours runs and never moves layout: the block is laid out line by line, so a build with no highlighter installed paints the same shape in one plain run.

The source is at `apps/gallery/src/patterns/syntax.rs`, and the highlighter it installs at `apps/gallery/src/highlight.rs`. Copy the file.
