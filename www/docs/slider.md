---
title: Slider
description: A filled track and a knob at a fraction — the paint only; the caller owns the value, the drag and what a key press is worth.
---

```rust
use ui::widgets::{self, Controls, SliderDrag};

focus::focusable(&theme, &self.slider, theme.slider(self.level))
    .id("slider")
    .on_drag(SliderDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
        view.level = widgets::axis_fraction(
            event.event.position, event.bounds, Axis::Horizontal, 0.0,
        );
        cx.notify();
    }))
```

The element *is* the drag source, so the gesture is grab-anywhere-and-slide rather than aim-at-the-knob.

## Keyboard

```rust
.on_action(cx.listener(|view, _: &focus::Decrement, _, cx| view.nudge(-STEP, cx)))
.on_action(cx.listener(|view, _: &focus::Increment, _, cx| view.nudge(STEP, cx)))
```

## API

| | |
| --- | --- |
| `slider(fraction)` | The track and knob. |
| `axis_fraction(pointer, bounds, axis, min)` | A slider passes `0.0` — it has no dead zone. |
| `SliderDrag` | A type of its own, so two sliders never answer each other's `on_drag_move`. |
| `focus::Decrement` / `Increment` | `←`/`→`. They carry no step: only the caller knows the range. |
