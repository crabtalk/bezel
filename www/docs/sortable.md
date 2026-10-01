---
title: Sortable lists
description: Drag items within and between host-laid-out regions, committed on drop.
---

`ui::drag` runs on gpui's active drag. The payload is `drag::Carry<ItemId>`; any element can join the same drag with `on_drag_move::<Carry<ItemId>>` and `on_drop::<Carry<ItemId>>`.

Keep one `drag::Domain<RegionId, ItemId>` on the view that renders all of its regions, initialized with `Domain::new(motion::Painter::of(cx))`. The host lays out, scrolls and virtualizes each region; the domain owns landing, displacement, edge scrolling and cancellation.

```rust
use gpui::Axis;
use ui::drag::Drop;

self.domain
    .region(("lane", lane.id), lane.id, Axis::Vertical,
        div().id(("cards", lane.id)).size_full().flex().flex_col().gap(px(8.))
            .overflow_y_scroll().track_scroll(&lane.scroll)
            .children(lane.cards.iter().map(|card| {
                self.domain.handle(card.id, self.card(card))
            })),
    )
    .track_scroll(&lane.scroll)
    .h(px(400.))
    .on_drop(cx.listener(|view, event: &Drop<LaneId, CardId>, _, cx| {
        view.move_card(event.item, event.region, event.after, event.before);
        cx.notify();
    }))
```

- `handle(item, element)` puts `on_drag` on `element`, which must not have one of its own. Item ids are unique within a domain. A handle is painted inside a region.
- `Drop { item, from, region, after, before }` names the landing by the painted neighbours on either side; one is `None` at an end of what was painted. Apply it synchronously. No callback fires for a cancelled drag or a release where the item started.
- Landings are computed only among handles painted in the last frame. A region's visible bounds are where it takes the pointer.
- `accepts(|item| …)` limits what a region takes. It takes every item by default.
- `lands(|item, after, before| …)` limits where in a region an item lands. The pointer's spot moves to the nearest allowed gap among painted items; with none allowed the region still holds the item and a release commits nothing.
- `carries(|item| …)` names the items that go along with `item` when it is picked up in that region, such as a heading's rows. They hide with it, and the gap it opens and closes is theirs too.
- `Domain::fixed(item, element)` is measured and moves aside like a handle but cannot be picked up.
- `feedback(Feedback::Indicator)` draws a line in the gap instead of moving neighbours. `Feedback::Displace`, the default, slides them aside and assumes every row between them is a handle.
- `track_scroll(&handle)` scrolls the region while the pointer rests near its visible edge (24px, or a quarter of a shorter region).
- `on_drop_outside` on the source region receives `Outside { item, position }` for a release over no accepting region.
- `Domain::with_ghost(painter, render)` hides the carried element in its slot and has gpui paint `render` under the pointer. `Domain::new` paints the carried element in place.
- Escape, `Domain::cancel`, or the drag ending elsewhere cancels without a drop. Custom controls inside a handle stop mouse-down propagation. Reduced motion skips slides.
- `Carry::claimed` is false while no region of the source domain holds the pointer; [docking](/docs/docking) takes the item only then.

[Tab strips](/docs/tab-strip) are one axis-locked region of this component.
