---
title: Resizable split
description: A hairline divider in a grab strip — the caller owns the fraction and reads the pointer through axis_fraction.
prompt: Split the window into a file tree on the left and the editor on the right, with a divider I can drag to resize.
---

```rust
use ui::widgets::{self, Layout as _, SplitDrag, SplitStyle};

div()
    .id("split")
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SplitDrag>, _, cx| {
        view.fraction = widgets::axis_fraction(event.event.position, event.bounds, Axis::Horizontal, 0.15);
        cx.notify();
    }))
    .child(div().w(relative(self.fraction)).child(left))
    .child(theme.split_handle(Axis::Horizontal, SplitStyle::Line { dragging: self.dragging })
        .id("split-handle")
        .on_drag(SplitDrag, |_, _, _, cx| cx.new(|_| gpui::Empty)))
    .child(div().flex_1().child(right))
```

- The fraction and the gesture are the caller's. Listen for `on_drag_move::<SplitDrag>` on the container that holds both panes.
- `min` clamps the fraction to `min..=1-min`. `SplitStyle::Ghost` takes the drag but paints nothing, for a pane that draws its own edge.
