---
title: Layout
description: Spacing steps, corner radii and chrome heights as plain numbers on Theme — no layout constant ever depends on which color is painted.
---

Law 4: numbers drive layout, colors are paint. Spacing and chrome heights are `f32` associated consts on `Theme` and the radii are `f32` functions, so a layout number resolves without reading the theme global at all:

```rust
div()
    .gap(px(Theme::SPACE_SM))
    .rounded(px(Theme::button_radius()))
    .h(px(Theme::HEADER_HEIGHT))
```

Spacing is `SPACE_XS` 4, `SPACE_SM` 8, `SPACE_MD` 12, `SPACE_LG` 16.

Radii are functions rather than consts, because every one of them is a ratio of `BASE_RADIUS` 8 and `Brand::radius` moves the whole set together. They run `control_radius()` 0.75x for things that sit inside a control, `button_radius()` 1x for controls themselves, `panel_radius()` 1.25x for cards, `surface_radius()` 1.5x for floating surfaces, `bubble_radius()` 2x for message bubbles. Chrome heights — `TITLEBAR_HEIGHT`, `HEADER_HEIGHT`, `STATUS_STRIP_HEIGHT` — are named for the same reason: a status strip that is reserved rather than conditional keeps the composer from shifting when it fills.

`surface_radius()` is read at both ends of a glass surface: the border paints it, and `ui::material` cuts the backdrop blur to it. A blur cut to a different radius frosts square corners outside a round border, visible only on glass and only at the corners — which is why the number is named once instead of written twice.

Nested corners come out of arithmetic rather than a second constant:

```rust
Theme::inset_radius(Theme::surface_radius(), 4.0) // 8.0
```

That is SwiftUI's `ContainerRelativeShape` rule. gpui has no container shape to inherit at paint time, so the relationship is stated where the child is defined — a container that changes its padding carries its rows with it, and the derived value never hardens into a constant of its own.
