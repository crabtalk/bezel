---
title: Pane docking
description: Tear tabs into panes with animated split and join previews.
---

`ui::docking::Dock<PaneId, ItemId>` handles tear-off, target previews, return
motion and settling. The host owns pane layout and tab membership.

Keep a controller on the view that renders the workspace:

```rust
let dock = docking::Dock::new(Painter::of(cx), |item, _, cx| {
    tabs::tab(Theme::of(cx), *item, tabs::Label::new(*item), tabs::State::Front)
        .into_any_element()
});
```

Mount the workspace in `dock.surface(id, child)`, and wrap each pane with
`dock.pane(pane_id, bar_height, child)`. Size the children normally; the wrappers
measure their window coordinates and visible bounds. Render participating
strips and panes in the controller's owning view.

The surface takes any gpui drag carrying a `drag::Carry<ItemId>` while
`Carry::claimed` is false: a tab more than 12px off its strip, or an item of a
[sortable](/docs/sortable) domain over no region that accepts it. While it
shows its ghost, the carried element is hidden. Returning to a region resumes
reordering there.

```rust
let owner = cx.entity().downgrade();
self.dock.surface("workspace", layout).on_drop(move |event, _, cx| {
    owner.update(cx, |view, cx| {
        // Apply the split or join and return the pane that received the item.
        let pane = view.apply_dock(&event.item, &event.pane, event.zone)?;
        cx.notify();
        Some(pane)
    }).ok().flatten()
})
```

The callback runs once on release with `Drop { item, pane, zone }`. Return
`None` to reject it. For an edge split, return the **new** pane's stable id;
for a join, return the existing pane. Apply accepted changes synchronously.
Remove empty panes and choose the active tab in host code.

`Zone::{Left, Right, Top, Bottom}` splits at the closest edge. The tab bar and
middle are `Zone::Join`. The public `zone` and `preview_bounds` helpers expose
the same geometry for custom targets.

The accent preview slides and resizes over 150ms. The resulting pane enters
from that rectangle as the ghost merges into it. Escape, a missing target or
a rejected drop returns the item home. Reduced motion snaps to final positions.
The ghost renderer is used while detached and during settling, within a
180×32px maximum frame.

A release off every region also reaches the source region's
`on_drop_outside`, if it has one.
