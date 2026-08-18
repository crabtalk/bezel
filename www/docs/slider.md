---
title: Slider
description: A filled track and a knob at a fraction — the paint only; the caller owns the value, the drag and what a key press is worth.
---

```rust
use ui::widgets::{self, SliderDrag};

focus::focusable(&theme, &self.slider, widgets::slider(&theme, self.level))
    .id("slider")
    .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
        view.level = widgets::axis_fraction(
            event.event.position, event.bounds, Axis::Horizontal, 0.0,
        );
        cx.notify();
    }))
```

The element *is* the drag source, so the gesture is grab-anywhere-and-slide rather than aim-at-the-knob. `axis_fraction` turns a pointer position into the value: where the pointer falls along an axis as a fraction of the bounds, clamped to `min..=1-min`. A slider passes `0.0` because it has no dead zone; a split passes one so neither pane can be squeezed away. On a zero-extent container — the frame before layout has run — it answers `min` instead of dividing by zero.

`SliderDrag` is a type of its own so two sliders in one window never answer each other's `on_drag_move`.

Keyboard is `←`/`→`, which arrive as `focus::Decrement` and `focus::Increment`:

```rust
.on_action(cx.listener(|view, _: &focus::Decrement, _, cx| view.nudge(-STEP, cx)))
.on_action(cx.listener(|view, _: &focus::Increment, _, cx| view.nudge(STEP, cx)))
```

The actions carry no step. Only the caller knows the range, and a library that picked one would be picking it for a percentage and a font size alike.
