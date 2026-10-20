---
title: Theme
description: Pick a hue, a chroma and a radius, watch every component repaint, and copy the code that reproduces it.
prompt: Give my app slate-tinted greys, a violet accent and slightly rounder corners.
---

```rust
use theme::{AppExt as _, Brand, Tint};

cx.set_brand(Brand {
    tint: Tint::new(257.417, 0.046),   // Slate, from theme::BASE_COLORS
    accent: Tint::new(276.935, 0.182),
    radius: 10.0,
    ..Brand::default()
});
```

- `Brand::default()` reproduces the shipped palette exactly; a `Tint` at chroma `0.0` is the shipped neutral.
- A brand rotates hue only. Tokens that carry their own hue (`danger`, `warning`, `success`), translucent ink, and palettes from a `ThemeFamily` keep theirs.
- An accent with chroma also moves `accent_strong`, and `on_accent` becomes whichever label holds on that plate.
- Every corner radius is a ratio of `radius`.
