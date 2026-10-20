---
title: Group box
description: The section card and the rows inside it — a bordered container that thins to a translucent tint over glass.
prompt: Build a settings page titled "Devices" with a card holding an Appearance row and a Storage row, each with an icon.
---

```rust
use ui::{icons::glyph, widgets::Scaffolding};

theme.page_column()
    .child(theme.page_header("Devices", Some(3)))
    .child(theme.page_subtitle("Signed in on three machines."))
    .child(theme.group_box()
        .child(theme.card_row(true)
            .child(theme.row_icon(glyph::Monitor))
            .child(theme.row_title("Appearance")))
        .child(theme.card_row(false)
            .child(theme.row_icon(glyph::Folder))
            .child(theme.row_title("Storage"))))
```

- Pass `first: true` to the opening row only; every other row draws a hairline above itself.
- Hover is the caller's. Chain one `.hover(..)` with `Theme::element_hover`; gpui panics on a second.
- An `option_card` preview that paints a background rounds itself to `OPTION_CARD_RADIUS`.
