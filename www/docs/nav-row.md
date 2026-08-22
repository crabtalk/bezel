---
title: Nav row
description: The sidebar row — a leading icon, a truncating label, and trailing content the caller owns.
---

```rust
use ui::{icons, widgets::Layout};

theme
    .nav_row(Some(icons::WIDGET), "Home", self.route == Route::Home, "nav-home")
    .id("nav-home")
    .on_click(cx.listener(|view, _, _, cx| view.go(Route::Home, cx)))
```

The label is a parameter rather than a child because it carries the truncation. Hand it out and the first long project name pushes the count and the chevron off the row instead of shortening itself.

Trailing content is the caller's: a count, a chevron, a control that appears under the pointer. That last one shares the row's `fade_key` — gpui allows one hover listener per element and the row has claimed it, so a trailing button paints its own tint with `motion::hover_blend` on the same key instead of adding an `on_hover` of its own.

Selection paints the wash `popover::menu_row` and `tree::tree_row` paint. A sidebar, a menu and a tree are three lists of the same kind, and they say "this one" the same way.

Two lines of text is a different row: `Scaffolding::row_title` over `meta_line`, inside a `card_row`.
