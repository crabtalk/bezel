---
title: Sheet
description: The dialog card pinned to a window edge — down the side or up from the bottom, same scrim, sliding in and back out on the popup's exit phase.
prompt: Slide a details panel in from the right when I click a row, and slide it back out when I click the dimmed page.
---

```rust
use ui::popover::{self, Side};

popover::sheet(
    "details",
    window.viewport_size(),
    Side::Right,
    px(320.0),
    popover::sheet_panel(&theme, Side::Right)
        .p(px(20.0))
        .child(popover::dialog_title(&theme, "Details"))
        .into_any_element(),
    self.sheet.closing_since(),
    cx.listener(|view, _, _, cx| popover::close_popup(view, cx, |view: &mut Self| &mut view.sheet)),
)
```

- Pass `closing_since()` and close through `popover::close_popup`, or the sheet is unmounted mid-slide.
- `extent` is a width on `Left` and `Right`, a height on `Bottom`.
- The scrim press is the last argument; `.on_mouse_down_out` from outside cannot reach it.
