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

The gesture stays with the caller because the fraction does. `split_handle` centres its line in a grab strip — the line plus 4px of slack each side, the same hitbox zed uses, because a 1px target is unhittable — and lights while `dragging`. The strip's width is `widgets::SPLIT_HANDLE_HIT`, for a caller laying out around it.

`SplitStyle::Ghost` takes the same drag and paints nothing, for a pane that already draws the edge itself. Two hairlines a pixel apart read as a seam rather than a divider.

`axis_fraction`'s last argument is the dead zone: `0.15` here keeps either pane from being squeezed away, clamping the answer to `0.15..=0.85`. On a zero-extent container, the frame before layout has run, it returns the minimum rather than dividing by zero.

`SplitDrag` is a distinct payload type so `on_drag_move::<SplitDrag>` on one container never fires for an unrelated split's gesture. `SliderDrag` exists for the same reason.
