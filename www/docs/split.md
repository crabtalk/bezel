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

| | |
| --- | --- |
| `split_handle(axis, style)` | The line centred in a grab strip; `SplitStyle::Line { dragging }` lights while held. |
| `SplitStyle::Ghost` | Takes the drag, paints nothing — for a pane that already draws the edge. |
| `SPLIT_HANDLE_HIT` | The strip's width: the line plus 4px of slack each side, since a 1px target is unhittable. |
| `axis_fraction(pointer, bounds, axis, min)` | `min` is the dead zone — `0.15` clamps to `0.15..=0.85`, and a zero-extent container answers `min` rather than dividing by zero. |
| `SplitDrag` | A distinct payload, so `on_drag_move::<SplitDrag>` never fires for an unrelated split. |
