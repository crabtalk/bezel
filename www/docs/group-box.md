---
title: Group box
description: The section card and the rows inside it — a bordered container that thins to a translucent tint over glass.
---

```rust
use ui::{icons, widgets::Scaffolding};

theme
    .group_box()
    .child(
        theme
            .card_row(true)
            .child(theme.row_icon(icons::glyph::Monitor))
            .child(theme.row_title("Appearance")),
    )
    .child(
        theme
            .card_row(false)
            .child(theme.row_icon(icons::glyph::Folder))
            .child(theme.row_title("Storage")),
    )
```

`first` suppresses the top hairline on the opening row — CSS writes that as `first:border-t-0`, and gpui has no sibling selectors.

## Page rhythm

```rust
theme.page_column()
    .child(theme.page_header("Devices", Some(3)))
    .child(theme.page_subtitle("Signed in on three machines."))
    .child(theme.field_label("Theme"))
```

## API

| | |
| --- | --- |
| `group_box()` | The plate. Fill is `Theme::card_glass_bg` — the opaque card tone, thinned to a tint over glass. |
| `card_row(first)` | One row, split from the one above by a hairline. |
| `row_icon(icon)` | The leading glyph, sized to the text beside it. |
| `row_title(title)` | Clipped rather than ellipsised, which keeps it in gpui's measure cache. |
| `meta_line(fragments)` | The quiet second line, joined by dots. |
| `page_column()` / `page_header(title, count)` / `page_subtitle(copy)` / `field_label(label)` | The rhythm around the card. |
| `option_card(label, selected, preview)` | A preview frame carrying the selection ring; the preview must round itself to `OPTION_CARD_RADIUS`. |

Hover is caller-owned — gpui panics on a second hover, so the wash to chain is `Theme::element_hover`.
