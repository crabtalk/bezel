---
title: Buttons
description: Buttons for gpui — default, prominent and destructive, each returning a plain gpui::Div you attach your own click handler to.
---

Three buttons, and the difference between them is only ever emphasis. Each returns a `gpui::Div`, so you wrap it in whatever handles the click — bezel does not own your interaction.

```rust
use bezel_ui::popover;

popover::button(&theme, "Cancel", "dialog-no")
popover::button_prominent(&theme, "Save")
popover::button_destructive(&theme, "Discard")
```

`button` takes a fade key. It identifies the button to the motion system so a hover that starts and a hover that ends belong to the same element across frames; two buttons sharing a key will trade one another's animation state.

The click handler is yours to attach:

```rust
div()
    .id("dialog-confirm")
    .on_click(cx.listener(|view, _, _, cx| view.close(cx)))
    .child(popover::button_prominent(&theme, "Save"))
```
