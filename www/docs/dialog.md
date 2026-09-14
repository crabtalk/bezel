---
title: Dialog
description: A centred card over a dim scrim, with the dialog-in entrance and the scrim press handed in as a parameter.
---

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

The scrim press is a parameter rather than the caller's `.on_mouse_down_out`, because the scrim lives *inside* this deferred layer and nothing outside can reach it.

## API

| | |
| --- | --- |
| `modal(id, viewport, card, on_scrim)` | `viewport_size` is required: an `anchored` layer sizes to its children, so the scrim needs explicit dimensions. |
| `modal_glass(..)` | For glass-tinted cards. Lighter scrim — the standard dim buries the backdrop hue and the card comes out a flat grey slab. |
| `dialog_card(theme)` / `dialog_title(theme, title)` / `dialog_body(theme, copy)` | The pieces to put in it. |

The card enters over `DIALOG_IN`. Which buttons, what they do and whether `esc` closes it are all the caller's.
