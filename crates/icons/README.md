# bezel-icons

[![crates.io](https://img.shields.io/crates/v/bezel-icons.svg?style=flat-square)](https://crates.io/crates/bezel-icons)
[![docs.rs](https://img.shields.io/docsrs/bezel-icons?style=flat-square)](https://docs.rs/bezel-icons)
[![license](https://img.shields.io/crates/l/bezel-icons.svg?style=flat-square)](../../LICENSE)

The [Lucide] set — 1834 glyphs, ported from a pinned release into typed
constants for [gpui]. Part of [bezel], and reached as `bezel::icons` from it.

```toml
[dependencies]
icons = { package = "bezel-icons", version = "0.1", features = ["navigation"] }
```

```rust
use icons::{Icon, navigation::Compass};

theme.icon(Compass)                          // ladder step, palette tone
theme.icon(Icon::glyph(Compass).solid())     // the filled variant
icons::icon(Compass).size(px(16.0)).text_color(theme.text_muted)
```

[`icon`] returns a gpui `Svg`, so it colors with `text_color` and sizes like any
element — but gpui reads an svg's tone from that element's own style and never
inherits one, so an icon with neither set paints nothing. `bezel-ui`'s
`theme.icon(..)` is this same builder with both supplied, which is what app code
should reach for; a component that has its own metric sets it on top.

## What a component takes

[`Icon`] is the value, and it erases where the drawing came from:

```rust
theme.row_icon(glyph::Monitor)                  // a glyph compiled in
theme.row_icon(Icon::asset("brand/mark.svg"))   // the app's own AssetSource
theme.row_icon(Icon::file(picked))              // a file on disk
```

Components take `impl Into<Icon>`, so a constant passes as itself and neither
the signature nor the component learns which it got. SwiftUI's `Image` erases
its sources the same way.

An `Icon` carries no size and no color. Those belong to the environment, which
here is the component: a menu row's glyph is the row's metric, not the caller's.
`Icon::solid` is the one thing it does carry — SwiftUI's `.symbolVariant(.fill)`
— and it fills the outline where [`icon`] draws it, so a control swapping
between them does not jump.

## Paying for what you paint

A constant is the SVG itself, not a path into an asset source — nothing
registers with `with_assets`, and the linker drops a glyph the app never names.

Features are Lucide's 42 categories under Lucide's own names, and `full` is all
of them. Nothing is on by default. Cargo unions features across a graph, so a
category a dependency turns on is the floor for everyone below it.

`CATEGORIES` is every enabled category and its icons, for an icon browser.
Naming it pins the enabled set, so an app that only paints icons never mentions
it and keeps the dead-code pass.

## Names

Modules are Lucide's categories and constants are Lucide's names, both generated
from the release: a name copied off [lucide.dev](https://lucide.dev) compiles
here. A glyph Lucide files under two categories is one constant re-exported
twice, so `glyph` holds each exactly once.

## Upgrading

`LUCIDE` in `build.rs` is the pinned release. Bumping it invalidates the index
beside the assets and the next build re-ports, so the diff shows which glyphs
upstream redrew. The port runs only in a fresh checkout — the published crate
carries the glyphs, which is what keeps it offline, vendorable and buildable on
docs.rs.

Icons are Lucide, under the [ISC license](https://lucide.dev/license).

[bezel]: https://github.com/crabtalk/bezel
[gpui]: https://github.com/zed-industries/zed/tree/main/crates/gpui
[lucide]: https://lucide.dev
[`icon`]: https://docs.rs/bezel-icons/latest/icons/fn.icon.html
[`solid`]: https://docs.rs/bezel-icons/latest/icons/fn.solid.html
[`Icon`]: https://docs.rs/bezel-icons/latest/icons/struct.Icon.html
