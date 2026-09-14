---
title: Layout
description: Spacing steps, corner radii and chrome heights as plain numbers on Theme — no layout constant ever depends on which color is painted.
---

```rust
div()
    .gap(px(Theme::SPACE))
    .rounded(px(Theme::button_radius()))
    .h(px(Theme::HEADER_HEIGHT))
```

Spacing and chrome heights are `f32` consts and the radii are `f32` functions, so a layout number resolves without reading the theme global at all.

## Nested corners

```rust
Theme::inset_radius(Theme::surface_radius(), 4.0) // 8.0
```

SwiftUI's `ContainerRelativeShape` rule. gpui has no container shape to inherit at paint time, so the relationship is stated where the child is defined.

## API

| | |
| --- | --- |
| `SPACE` | 8 — one number, not a ladder. `ui::stack::row` and `column` carry it, so a call site wanting the standard gap writes none. |
| `CONTENT_MARGIN` | 20, content to its container's edge. |
| `control_radius()` | 0.75× — things that sit inside a control. |
| `button_radius()` | 1× — controls themselves. |
| `panel_radius()` | 1.25× — cards. |
| `surface_radius()` | 1.5× — floating surfaces. Read at both ends of glass: the border paints it and `ui::surface` cuts the blur to it. |
| `bubble_radius()` | 2× — message bubbles. |
| `TITLEBAR_HEIGHT` / `HEADER_HEIGHT` / `STATUS_STRIP_HEIGHT` | Reserved rather than conditional, so a strip filling never shifts the composer. |

Every radius is a ratio of `BASE_RADIUS` 8, so `Brand::radius` moves the whole set together and keeps the concentric relationships intact.
