---
title: Resizable split
description: A hairline divider in a grab strip — the caller owns the fraction and reads the pointer through axis_fraction.
---

```rust
use ui::widgets::{self, Layout as _, SplitDrag, SplitStyle};

div()
    .id("split")
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SplitDrag>, _, cx| {
        view.fraction = widgets::axis_fraction(
            event.event.position, event.bounds, Axis::Horizontal, 0.15,
        );
        cx.notify();
    }))
    .child(div().w(relative(self.fraction)).child(left))
    .child(
        theme
            .split_handle(Axis::Horizontal, SplitStyle::Line { dragging: self.dragging })
            .id("split-handle")
            .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| gpui::Empty)),
    )
    .child(div().flex_1().child(right))
```

The gesture stays with the caller because the fraction does.

## API

```rust
pub trait Layout: ThemeExt {
    /// The line centred in a grab strip; `Line { dragging }` lights while held,
    /// and `Ghost` takes the drag but paints nothing.
    fn split_handle(&self, axis: gpui::Axis, style: SplitStyle) -> Div;

    // ...
}

/// The strip's width: the line plus 4px of slack each side, since a 1px target
/// is unhittable.
pub const SPLIT_HANDLE_HIT: f32;

/// A distinct payload, so `on_drag_move::<SplitDrag>` never fires for an
/// unrelated split.
pub struct SplitDrag;

/// `min` is the dead zone — `0.15` clamps to `0.15..=0.85`. A zero-extent
/// container answers `min` rather than dividing by zero.
pub fn axis_fraction(
    pointer: gpui::Point<gpui::Pixels>,
    bounds: gpui::Bounds<gpui::Pixels>,
    axis: gpui::Axis,
    min: f32,
) -> f32;
```
