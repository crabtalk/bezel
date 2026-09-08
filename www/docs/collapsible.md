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

The header is a row, not a container. Swallowing the children would mean re-implementing layout for them, and the body of a collapsible is usually the most layout-specific thing on the page.

`theme.disclosure(expanded)` is the chevron on its own — two assets rather than one rotated, because gpui has no transform for `div`s at the pinned rev.

When the section should open itself while something runs and close when it stops, `Takeover` is the flag:

```rust
let open = self.details.get(self.running);  // auto until touched
self.details.toggle(self.running);          // the click wins from here
```

Auto-follow is right until the first press and wrong immediately after — whatever the flag does next, the person who clicked has to win. Nothing agent-shaped about it: a build log that unfolds while it runs and a detail pane that follows the selection both want exactly this.
