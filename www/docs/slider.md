---
title: Slider
description: A filled track and a knob at a fraction — the paint only; the caller owns the value, the drag and what a key press is worth.
prompt: Add a volume slider that I can drag or nudge with the arrow keys in 5% steps.
---

```rust
use ui::{focus, widgets::{self, Controls, SliderDrag}};

focus::focusable(&theme, &self.slider, theme.slider(self.level))
    .id("volume")
    .on_drag(SliderDrag("volume".into()), |_, _, _, cx| cx.new(|_| gpui::Empty))
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
        if let Some(fraction) = widgets::slider_fraction(event, "volume", cx) {
            view.level = fraction;
            cx.notify();
        }
    }))
    .on_action(cx.listener(|view, _: &focus::Decrement, _, cx| view.nudge(-0.05, cx)))
    .on_action(cx.listener(|view, _: &focus::Increment, _, cx| view.nudge(0.05, cx)))
```

- The id in `SliderDrag` and in `slider_fraction` must be the element's own id; every slider hears every slider's drag.
- `focus::Decrement` / `Increment` (`←` / `→`) carry no step. The caller picks it.
