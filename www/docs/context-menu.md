---
title: Context menu
description: The same menu card floated at an explicit window point — right-click position in, occluding layer out.
---

A context menu is `menu_at`: the card, positioned by a point rather than by a trigger.

```rust
use ui::popover;

.on_mouse_down(MouseButton::Right, cx.listener(|view, event: &MouseDownEvent, _, cx| {
    view.context_menu.open(event.position);
    cx.notify();
}))
```

Then render it while the popup holds a position:

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

The last argument is the `Popup`'s `closing_since()`. Pass it and the menu plays `menu-out` on the way away; pass `None` and it disappears the frame its state drops.

Like every floating layer here it occludes, so rows never leak their clicks to the elements underneath. Dismissal is still the caller's `.on_mouse_down_out` — nothing in the library decides when your menu should go away.
