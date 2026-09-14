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

```rust
impl Theme {
    /// One number, not a ladder. `ui::stack::row` and `column` carry it, so a
    /// call site wanting the standard gap writes none.
    pub const SPACE: f32 = 8.0;

    /// Content to its container's edge.
    pub const CONTENT_MARGIN: f32 = 20.0;

    // Reserved rather than conditional, so a strip filling never shifts the
    // composer.
    pub const TITLEBAR_HEIGHT: f32 = 38.0;
    pub const HEADER_HEIGHT: f32 = 44.0;
    pub const STATUS_STRIP_HEIGHT: f32 = 24.0;

    /// Every radius below is a ratio of this, so `Brand::radius` moves the
    /// whole set together and keeps the concentric relationships intact.
    pub const BASE_RADIUS: f32 = 8.0;

    /// 0.75x — things that sit inside a control.
    pub fn control_radius() -> f32;
    /// 1x — controls themselves.
    pub fn button_radius() -> f32;
    /// 1.25x — cards.
    pub fn panel_radius() -> f32;
    /// 1.5x — floating surfaces. Read at both ends of glass: the border paints
    /// it and `ui::surface` cuts the blur to it.
    pub fn surface_radius() -> f32;
    /// 2x — message bubbles.
    pub fn bubble_radius() -> f32;

    /// SwiftUI's `ContainerRelativeShape` rule, stated where the child is.
    pub const fn inset_radius(outer: f32, inset: f32) -> f32;

    // ...
}
```

Every radius is a ratio of `BASE_RADIUS` 8, so `Brand::radius` moves the whole set together and keeps the concentric relationships intact.
