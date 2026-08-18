---
title: Icons
description: 79 SVGs embedded in the crate and served to gpui through an AssetSource, each one a &'static str path.
---

Register the asset source when the app starts, then name an icon by its constant:

```rust
use ui::icons;

Application::new().with_assets(icons::Assets)

icons::icon(icons::PAPERCLIP).size(px(16.0)).text_color(theme.text_muted)
```

`icon` returns a gpui `Svg`, so it colors with `text_color` and sizes like any element. The paths are `&'static str` — that is what makes the set browsable: `icons::ALL` is every `(constant name, asset path)` pair, which is what this page renders.

Most glyphs are **Solar Icons** (Linear weight) by 480 Design, licensed CC BY 4.0. A handful — `TERMINAL`, `PLUS`, `CLOSE`, `STOP`, `GIT_BRANCH`, `STAR` — are drawn to match, because the set has no equivalent.

`SIDEBAR_MINIMALISTIC_LEFT` is the mirrored twin of `SIDEBAR_MINIMALISTIC` rather than a transform at the call site: gpui divs have no scale transform at the pinned rev, so the flip is baked into the asset.
