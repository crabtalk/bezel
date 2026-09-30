---
title: Sortable lists
description: Animated reordering along either axis and between lists, committed on drop.
---

Keep one `sortable::Sortable<ListId, ItemId>` per drag domain, initialized with
`Sortable::new(motion::Painter::of(cx))`. The host owns the data; the component
owns the gesture, temporary order, gap, animation, and edge scrolling.

```rust
use gpui::{Axis, px};
use ui::sortable::{List, Move};

self.sortable.group("board", self.lanes.iter().map(|lane| {
    List::new(self.element_key(lane), lane.id, Axis::Vertical,
        lane.cards.iter().map(|card| (card.id, self.card(card))),
    )
    .kind("cards")
    .gap(px(8.))
    .w(px(240.))
    .h(px(400.))
    .track_scroll(&lane.scroll)
    .header(self.heading(lane))
})).on_drop(cx.listener(|view, event: &Move<LaneId, CardId>, _, cx| {
    let card = view.lanes[event.from.list].cards.remove(event.from.index);
    view.lanes[event.to.list].cards.insert(event.to.index, card);
    cx.notify();
}))
```

- List ids and item ids must be stable. Item ids are unique across the group.
- Supply all participating lists together, including empty ones. Give empty
  lists a visible size so they can accept a drop.
- `from.index` addresses the original source. `to.index` is the insertion index
  **after removing the source item**, including for a move within one list.
- Apply a move synchronously. No callback fires for a no-op or cancelled move.
  The host order stays unchanged until release.
- `Axis::Horizontal` and `Axis::Vertical` use the same gesture. The group lays
  lists in a row by default; style it with the usual flex methods.
- Only visible viewport bounds accept drops. A held item near a list edge
  scrolls that list, and scrolling updates the target without pointer movement.
- `header` stays outside the scrolling area. It is not a drop target.

Escape, releasing outside every compatible list, or host edits to list
membership, order, or kind cancel the preview. `cancel(window, cx)` cancels explicitly. An optional
`on_drop_outside` receives the item and window position for external routing,
including releases over an incompatible list. Escape and host edits do not
emit an outside drop.
Use `drag_style` for an additional surface or shadow on the carried item.
Custom controls inside an item should stop mouse-down propagation to avoid
starting a drag. Reduced motion skips slides.

## Item kinds

Lists exchange items only when their `.kind(...)` tags match by value. Untagged
lists share the empty tag. Incompatible lists open no gap and do not edge-scroll;
the item continues following the pointer past them.

A sidebar can use `"project-headings"` and `"space-headings"` for its heading
lists, and `.kind(format!("entries:{group_id}"))` to keep entry rows within their
own group. Use a shared entry kind instead when moves between groups are allowed.
An enum can represent the different item ids; ids remain unique across the group.

Lists measure mounted children, including varying sizes; this is not a
virtualized-list adapter. [Tab strips](/docs/tab-strip) use this same component
with one horizontal list and movement constrained to its axis.

Attach `.docking(&dock)` to hand items outside compatible lists to
[a pane docking surface](/docs/docking). Local list drops keep `on_drop`;
the docking surface handles detached releases.
