---
title: Icons
description: 84 icons declared from Lucide a line each, grouped into categories and served to gpui through an AssetSource.
---

Register the asset source when the app starts, then name an icon by its category and constant:

```rust
use ui::icons::{self, system};

Application::new().with_assets(icons::Assets)

icons::icon(system::MAGNIFER).size(px(16.0)).text_color(theme.text_muted)
```

`icon` returns a gpui `Svg`, so it colors with `text_color` and sizes like any element. The paths are `&'static str` — that is what makes the set browsable: `icons::CATEGORIES` is every category and its `(constant name, asset path)` pairs, which is what this page renders.

## Nothing is checked in

No SVG lives in the repository. The set is declared in `crates/icons/src/lib.rs`, a line per icon naming ours and the [Lucide](https://lucide.dev) glyph behind it:

```rust
icon_set! {
    /// Transport, and the three volume glyphs a level control swaps between.
    media = "media" {
        (PLAY, "play", LuPlay),
        (PLAY_BOLD, "play-bold", LuPlay, solid),
    }
}
```

`icon_set!` is a `macro_rules!` in that same file, expanding the declaration into the modules, the constants and the `AssetSource`; `document`, also there, turns a glyph into the SVG gpui asks for. Nothing is generated out of band — rustfmt formats it, rust-analyzer expands it, and `LuPlay` is resolved by the compiler, so a glyph that does not exist upstream is an error naming the bad identifier rather than a blank square at runtime.

An icon writes its name twice because building an identifier out of a string is the one thing only a procedural macro can do, and that is not worth a second published crate. A test asserts the two halves agree, which is the drift that matters: the older vendored set had grown a `DOWNLOAD` constant pointing at `download-minimalistic.svg`.

Nothing is fetched while building — cargo has `icondata_lu` in its registry cache before rustc starts — so the set works offline, under `cargo vendor` and on docs.rs, pinned byte-for-byte by `Cargo.lock`. We name 84 of that crate's 1599 statics and the linker drops the rest.

Lucide is ISC-licensed and asks for no attribution in a shipped binary.

## Paying for what you paint

Each category — `arrows`, `media`, `files`, `devices`, `editing`, `status`, `system` — is a module and a cargo feature, all on by default. `default-features = false` narrows the set, and the icons you leave out are neither generated nor embedded:

```toml
bezel-icons = { version = "0.1", default-features = false, features = ["arrows", "system"] }
```

Cargo unions features across a graph, so a category a dependency turns on is one you cannot turn off. `ui` asks for four of the seven to compile its own components, and those four are the floor for anything depending on it.

## Bold twins

Lucide draws one weight. The `_BOLD` icons — `media::PLAY_BOLD`, `status::STAR_BOLD` — are the same path painted solid, filled *and* stroked so the outer edge lands exactly where the outline twin's does. A control swapping between them does not jump.
