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

```rust
// ui::popover

/// `viewport` is required: an `anchored` layer sizes to its children, so the
/// scrim needs explicit dimensions.
pub fn modal(
    id: impl Into<ElementId>,
    viewport: gpui::Size<Pixels>,
    card: AnyElement,
    on_dismiss: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement;

/// For glass-tinted cards. Lighter scrim — the standard dim buries the
/// backdrop hue and the card comes out a flat grey slab.
pub fn modal_glass(/* the same */) -> AnyElement;

// The pieces to put in it.
pub fn dialog_card(theme: &Theme) -> gpui::Div;
pub fn dialog_title(theme: &Theme, title: impl Into<SharedString>) -> gpui::Div;
pub fn dialog_body(theme: &Theme, copy: impl Into<SharedString>) -> gpui::Div;

// ...
```

The card enters over `DIALOG_IN`. Which buttons, what they do and whether `esc` closes it are all the caller's.
