---
title: Group box
description: The section card and the rows inside it — a bordered container that thins to a translucent tint over glass.
---

```rust
use bezel_ui::{icons, widgets};

widgets::group_box(&theme)
    .child(
        widgets::card_row(&theme, true)
            .child(widgets::row_tile(&theme, icons::MONITOR))
            .child(widgets::row_title(&theme, "Appearance")),
    )
    .child(
        widgets::card_row(&theme, false)
            .child(widgets::row_tile(&theme, icons::FOLDER))
            .child(widgets::row_title(&theme, "Storage")),
    )
```

`card_row`'s `first` flag is what suppresses the top hairline on the row that opens the card — CSS would write that as `first:border-t-0`, and gpui has no sibling selectors, so the caller says which row is first.

The card's fill comes from `Theme::card_glass_bg`: the opaque card tone on an opaque appearance, thinned to a translucent tint over glass, where the solid tone read as a slab floating on the frosted blur.

Around it, the page rhythm is `page_column` (a centred reading column), `page_header` for the headline and its count sharing a baseline, `page_subtitle`, and `field_label` for the small caption over a control.
