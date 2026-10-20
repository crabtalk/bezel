---
title: Dialog
description: A centred card over a dim scrim, with the dialog-in entrance and the scrim press handed in as a parameter.
prompt: Ask "Discard changes?" in a dialog with Cancel and Discard buttons that also closes when I click outside it.
---

```rust
use ui::popover;

popover::modal(
    "discard",
    window.viewport_size(),
    popover::dialog_card(&theme)
        .gap(px(12.0))
        .child(popover::dialog_title(&theme, "Discard changes?"))
        .child(popover::dialog_body(&theme, "This cannot be undone."))
        .child(div().flex().justify_end().gap(px(8.0)).child(cancel).child(discard))
        .into_any_element(),
    cx.listener(|view, _, _, cx| view.close_dialog(cx)),
)
```

- The scrim press is the last argument; `.on_mouse_down_out` from outside cannot reach it.
- The buttons, and whether `escape` closes it, are the caller's.
- No exit animation: a `Popup` holding it closes with `Popup::close`.
- `modal_glass` is the lighter scrim for a glass-tinted card.
