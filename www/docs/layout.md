---
title: Layout
description: Spacing steps, corner radii and chrome heights as plain numbers on Theme — no layout constant ever depends on which color is painted.
---

Law 4: numbers drive layout, colors are paint. The layout constants are `f32` associated consts on `Theme`, so they resolve without reading the global at all:

```rust
div()
    .gap(px(Theme::SPACE_SM))
    .rounded(px(Theme::BUTTON_RADIUS))
    .h(px(Theme::HEADER_HEIGHT))
```

Spacing is `SPACE_XS` 4, `SPACE_SM` 8, `SPACE_MD` 12, `SPACE_LG` 16. Radii run `CONTROL_RADIUS` 6 for things that sit inside a control, `BUTTON_RADIUS` 8 for controls themselves, `PANEL_RADIUS` 10 for cards, `SURFACE_RADIUS` 12 for floating surfaces, `BUBBLE_RADIUS` 16 for message bubbles. Chrome heights — `TITLEBAR_HEIGHT`, `HEADER_HEIGHT`, `STATUS_STRIP_HEIGHT` — are named for the same reason: a status strip that is reserved rather than conditional keeps the composer from shifting when it fills.

`SURFACE_RADIUS` is read at both ends of a glass surface: the border paints it, and `ui::material` cuts the backdrop blur to it. A blur cut to a different radius frosts square corners outside a round border, visible only on glass and only at the corners — which is why the number is named once instead of written twice.

Nested corners come out of arithmetic rather than a second constant:

```rust
Theme::inset_radius(Theme::SURFACE_RADIUS, 4.0) // 8.0
```

That is SwiftUI's `ContainerRelativeShape` rule. gpui has no container shape to inherit at paint time, so the relationship is stated where the child is defined — a container that changes its padding carries its rows with it, and the derived value never hardens into a constant of its own.
