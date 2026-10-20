---
title: Nav row
description: The sidebar row — a leading icon, a truncating label, and trailing content the caller owns.
prompt: Build a sidebar with Home, Projects and Settings rows that highlight the current route and navigate on click.
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

- The `Fade` must be stable across frames.
- The row owns its hover. A trailing control that shows on hover paints with `motion::hover_blend` on the row's fade, not an `on_hover` of its own.
