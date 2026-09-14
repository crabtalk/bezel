---
title: Nav row
description: The sidebar row — a leading icon, a truncating label, and trailing content the caller owns.
---

```rust
use motion::{Fade, Painter};
use ui::{icons, widgets::Layout};

theme
    .nav_row(
        Some(icons::glyph::LayoutGrid),
        "Home",
        self.route == Route::Home,
        Fade::new(Painter::of(cx), "nav-home"),
    )
    .id("nav-home")
    .on_click(cx.listener(|view, _, _, cx| view.go(Route::Home, cx)))
```

The label is a parameter rather than a child because it carries the truncation — hand it out and the first long project name pushes the count and the chevron off the row.

## API

| | |
| --- | --- |
| `nav_row(icon, label, selected, fade)` | Selected paints the wash `menu_row` and `tree_row` paint. |

A trailing control that appears on hover shares the row's `Fade` through `motion::hover_blend`: gpui allows one hover listener per element, and the row has claimed it. Two lines of text is a different row — `row_title` over `meta_line`, inside a [`card_row`](/docs/group-box).
