---
title: Pane docking
description: Tear tabs into panes with animated split and join previews.
prompt: Let me drag a tab out of its pane and drop it on an edge of another pane to split it, or on its tab bar to join.
---

```rust
let dock = docking::Dock::new(Painter::of(cx), |item, _, cx| {
    tabs::tab(Theme::of(cx), *item, tabs::Label::new(*item), tabs::State::Front).into_any_element()
});

let owner = cx.entity().downgrade();
self.dock.surface("workspace", layout).on_drop(move |event, _, cx| {
    owner.update(cx, |view, cx| {
        let pane = view.apply_dock(&event.item, &event.pane, event.zone)?;
        cx.notify();
        Some(pane)
    }).ok().flatten()
})
```

- Keep one `Dock` on the view that renders the surface, its strips and its panes. Wrap each pane in `dock.pane(pane_id, bar_height, child)`.
- `on_drop` returns the new pane's stable id for an edge split and the existing pane for a join; `None` sends the item home. Apply the change synchronously.
- Empty panes and the active tab are the host's. Strips are [tab strips](/docs/tab-strip) or [sortable](/docs/sortable) regions of the same item type.
