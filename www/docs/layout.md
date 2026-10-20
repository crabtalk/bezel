---
title: Layout
description: Spacing steps, corner radii and chrome heights as plain numbers on Theme — no layout constant ever depends on which color is painted.
prompt: Make the settings cards use the theme spacing and corner radii, with the rows inside nested concentrically.
---

```rust
use theme::{ControlSize, Sizing as _, Theme};

let pad = 4.0;
div()
    .p(px(pad))
    .rounded(px(Theme::surface_radius()))
    .child(div().rounded(px(Theme::inset_radius(Theme::surface_radius(), pad))))

div().h(px(Theme::HEADER_HEIGHT)).gap(px(Theme::SPACE))
div().control_size(ControlSize::Small)
```

- Radii are functions of `Brand::radius`, read process-wide. Call them where you paint; a radius stored in a `const` misses a brand change.
- A rounded container does not pass its corner to its children; a child inset by `pad` takes `inset_radius(outer, pad)`.
- `ui::stack::row` and `column` already carry `Theme::SPACE`.
