---
title: Collapsible
description: A disclosure header — chevron plus title — with the body rendered by the caller, and a flag that can follow something until you take it over.
---

```rust
use ui::widgets::Layout;

div()
    .child(
        div()
            .id("collapse")
            .on_click(cx.listener(|view, _, _, cx| {
                view.expanded = !view.expanded;
                cx.notify();
            }))
            .child(theme.collapsible_header("Advanced", self.expanded)),
    )
    .when(self.expanded, |el| el.child(body))
```

The header is a row, not a container: the body of a collapsible is usually the most layout-specific thing on the page.

## Following a run

```rust
let open = self.details.get(self.running);  // auto until touched
self.details.toggle(self.running);          // the click wins from here
```

## API

```rust
pub trait Layout: ThemeExt {
    /// Chevron plus title.
    fn collapsible_header(&self, label: impl Into<SharedString>, expanded: bool) -> Div;

    /// The chevron alone — two assets, since gpui has no `div` transform at
    /// the pinned rev.
    fn disclosure(&self, expanded: bool) -> Svg;

    // ...
}

impl Takeover {
    /// Auto-follow until the first press, the user's choice after.
    pub fn get(self, auto: bool) -> bool;
    pub fn toggle(&mut self, auto: bool);
}
```
