---
title: Slider
description: A filled track and a knob at a fraction — the paint only; the caller owns the value, the drag and what a key press is worth.
---

```rust
use ui::widgets::{self, Controls, SliderDrag};

focus::focusable(&theme, &self.slider, theme.slider(self.level))
    .id("slider")
    .on_drag(SliderDrag("slider".into()), |_, _, _, cx| cx.new(|_| gpui::Empty))
    .on_drag_move(cx.listener(|view, event: &DragMoveEvent<SliderDrag>, _, cx| {
        let Some(fraction) = widgets::slider_fraction(event, "slider", cx) else {
            return;
        };
        view.level = fraction;
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

```rust
pub trait Controls: ThemeExt {
    fn slider(&self, fraction: f32) -> Div;

    // ...
}

/// Where the drag lands on the track it is asked about, or `None` when the
/// gesture belongs to another slider.
pub fn slider_fraction(
    event: &DragMoveEvent<SliderDrag>,
    id: impl Into<ElementId>,
    cx: &App,
) -> Option<f32>;

/// A type of its own, so two sliders never answer each other's `on_drag_move`;
/// the id inside it is which one the gesture started on.
pub struct SliderDrag(pub ElementId);
```

`focus::Decrement` and `focus::Increment` are `←`/`→`. They carry no step: only the caller knows the range.
