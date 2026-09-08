---
title: Layout
description: Spacing steps, corner radii and chrome heights as plain numbers on Theme — no layout constant ever depends on which color is painted.
---

Law 4: numbers drive layout, colors are paint. Spacing and chrome heights are `f32` associated consts on `Theme` and the radii are `f32` functions, so a layout number resolves without reading the theme global at all:

```rust
div()
    .gap(px(Theme::SPACE))
    .rounded(px(Theme::button_radius()))
    .h(px(Theme::HEADER_HEIGHT))
```

Spacing is one number, not a ladder: `SPACE` 8, measured on macOS 26 where `NSStackView().spacing`, visual format's `-` and `constraint(equalToSystemSpacingAfter:)` all report it, with `CONTENT_MARGIN` 20 the same measurement from content to its container's edge. `ui::stack::row` and `ui::stack::column` carry it, so a call site that wants the standard gap writes no number at all — SwiftUI's shape, where `VStack(spacing:)` takes the system's when given nothing.

Radii are functions rather than consts, because every one of them is a ratio of `BASE_RADIUS` 8 and `Brand::radius` moves the whole set together. They run `control_radius()` 0.75x for things that sit inside a control, `button_radius()` 1x for controls themselves, `panel_radius()` 1.25x for cards, `surface_radius()` 1.5x for floating surfaces, `bubble_radius()` 2x for message bubbles. Chrome heights — `TITLEBAR_HEIGHT`, `HEADER_HEIGHT`, `STATUS_STRIP_HEIGHT` — are named for the same reason: a status strip that is reserved rather than conditional keeps the composer from shifting when it fills.

`surface_radius()` is read at both ends of a glass surface: the border paints it, and `ui::surface` cuts the backdrop blur to it. A blur cut to a different radius frosts square corners outside a round border, visible only on glass and only at the corners — which is why the number is named once instead of written twice.

Nested corners come out of arithmetic rather than a second constant:

```rust
Theme::inset_radius(Theme::surface_radius(), 4.0) // 8.0
```

That is SwiftUI's `ContainerRelativeShape` rule. gpui has no container shape to inherit at paint time, so the relationship is stated where the child is defined — a container that changes its padding carries its rows with it, and the derived value never hardens into a constant of its own.
