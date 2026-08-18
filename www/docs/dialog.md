---
title: Dialog
description: A centred card over a dim scrim, with the dialog-in entrance and the scrim press handed in as a parameter.
---

`modal` is the scrim and the layer; `dialog_card` and its pieces are what you put in it:

```rust
use ui::popover;

popover::modal(
    "gallery-dialog",
    window.viewport_size(),
    popover::dialog_card(&theme)
        .gap(px(12.0))
        .child(popover::dialog_title(&theme, "Discard changes?"))
        .child(popover::dialog_body(&theme, "This cannot be undone."))
        .child(
            div().flex().flex_row().justify_end().gap(px(8.0))
                .child(cancel_button)
                .child(confirm_button),
        )
        .into_any_element(),
    cx.listener(|view, _, _, cx| view.close_dialog(cx)),
)
```

`viewport_size` is required: an `anchored` layer sizes to its children, so the scrim needs explicit dimensions to cover the window.

The last argument is the scrim press, and it is a parameter rather than the caller's `.on_mouse_down_out` because the scrim lives *inside* this deferred layer — nothing outside can reach it. That is not hypothetical: the first version shipped without it, and what it looked like was a dialog that only closed on its own buttons.

`modal_glass` is the variant for glass-tinted cards. Its scrim is lighter, because the standard dim buries the backdrop hue under the blur and the card comes out a flat grey slab next to the hue-inheriting menus. Its radius is not a parameter — a glass-tinted modal *is* a popover surface, and the parameter it used to take carried the doc line "must match the card's rounding", which is a footgun handed to the caller in writing.

The card enters over `DIALOG_IN`. Everything else — which buttons, what they do, whether `esc` closes it — is the caller's.
