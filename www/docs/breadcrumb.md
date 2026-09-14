---
title: Breadcrumb
description: A trail of crumbs and chevrons, assembled by the caller — the last one is current and stops looking clickable.
---

```rust
use ui::widgets::Content;

theme
    .breadcrumb()
    .child(theme.breadcrumb_item("crates", false))
    .child(theme.breadcrumb_separator())
    .child(theme.breadcrumb_item("ui", false))
    .child(theme.breadcrumb_separator())
    .child(theme.breadcrumb_item("widgets.rs", true))
```

Separators are children rather than something the container inserts, so collapsing a long trail to `crates / … / widgets.rs` stays the caller's decision.

## API

```rust
pub trait Content: ThemeExt {
    /// The row. `min_w_0`, so a long path shortens instead of widening it.
    fn breadcrumb(&self) -> Div;

    /// One crumb; `current` takes the text tone and drops the pointer cursor.
    fn breadcrumb_item(&self, label: impl Into<SharedString>, current: bool) -> Div;

    /// The chevron between two.
    fn breadcrumb_separator(&self) -> Svg;

    // ...
}
```
