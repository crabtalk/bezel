---
title: Context menu
description: The same menu card floated at an explicit window point — right-click position in, occluding layer out.
---

```rust
use ui::popover;

.on_mouse_down(MouseButton::Right, cx.listener(|view, event: &MouseDownEvent, _, cx| {
    view.context_menu.open(event.position);
    cx.notify();
}))
```

```rust
popover::menu_at(
    "gallery-context",
    position,
    popover::popover_card(&theme)
        .w(px(180.0))
        .children(rows)
        .on_mouse_down_out(cx.listener(|view, _, _, cx| view.close_context_menu(cx)))
        .into_any_element(),
    closing,
)
```

## API

```rust
// ui::popover

/// The card positioned by a point rather than by a trigger. `closing` is a
/// `Popup`'s `closing_since()`, which plays `menu-out` on the way away;
/// `None` disappears the frame the state drops.
pub fn menu_at(
    id: impl Into<SharedString>,
    position: Point<Pixels>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement;

// ...
```

Dismissal is the caller's `.on_mouse_down_out` — nothing here decides when your menu should go away.
