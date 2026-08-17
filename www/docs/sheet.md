---
title: Sheet
description: The dialog card pinned to a window edge — full height, same scrim, sliding in and back out on the popup's exit phase.
---

```rust
use bezel_ui::popover::{self, Side};

popover::sheet(
    "gallery-sheet",
    window.viewport_size(),
    Side::Right,
    px(320.0),
    popover::sheet_panel(&theme, Side::Right)
        .p(px(20.0))
        .child(popover::dialog_title(&theme, "Details"))
        .child(popover::dialog_body(&theme, "…"))
        .into_any_element(),
    self.sheet.closing_since(),
    cx.listener(|view, _, _, cx| view.close_sheet(cx)),
)
```

`sheet_panel` rounds and hairlines its *inner* edge only — the two corners on the window edge are off screen — so the panel reads as pulled out of the side of the window rather than floating near it. It shares one rounding constant with `dialog_card`, because a sheet is the dialog card pinned to an edge, and that number is read three times over: the card, the panel, and the blur under each.

It slides in over `DIALOG_IN` and back out over `MENU_OUT`. The exit is not optional — `Popup::finish_close` reaps on that spec's span, so a sheet that ignored `closing_since` would be unmounted mid-slide.

As with `modal`, the scrim press is a parameter. The scrim is inside the deferred layer, so `.on_mouse_down_out` from outside can never reach it.

The slide itself is written in the component rather than as a motion helper: only the *spec* is motion, and which inset carries it is layout that differs per side.
