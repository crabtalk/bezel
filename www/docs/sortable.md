---
title: Sortable lists
description: Drag items within and between host-laid-out regions, committed on drop.
prompt: Make the board's cards draggable within a lane and between lanes, and save the move when I drop.
---

```rust
self.domain   // drag::Domain<LaneId, CardId>, Domain::new()
    .region(("lane", lane.id), lane.id, Axis::Vertical,
        div().id(("cards", lane.id)).size_full().flex().flex_col().gap(px(8.))
            .overflow_y_scroll().track_scroll(&lane.scroll)
            .children(lane.cards.iter().map(|card| self.domain.handle(card.id, self.card(card)))),
    )
    .track_scroll(&lane.scroll)
    .h(px(400.))
    .on_drop(cx.listener(|view, event: &drag::Drop<LaneId, CardId>, _, cx| {
        view.move_card(event.item, event.region, event.after, event.before);
        cx.notify();
    }))
```

- Keep one `Domain` on the view that renders its regions. Item ids are unique within it, and a handle's element has no `on_drag` of its own.
- `Drop` names the landing by the painted neighbours `after` and `before`; either is `None` at an end. Apply it synchronously.
- Landings are found only among handles painted in the frame, and a region takes the pointer over its visible bounds.
- A control inside a handle stops mouse-down propagation.

## Make it yours

| call | what it decides |
| --- | --- |
| `accepts` | which items a region takes |
| `lands` | where in a region an item may land |
| `feedback(Feedback::Indicator)` | a line in the gap instead of sliding neighbours |
| `Domain::fixed` | a row that moves aside but can't be picked up |
| `Domain::with_ghost` | a ghost under the pointer, for virtualized regions |
| `on_drop_outside` | a release over no accepting region |
| `axis_locked` | an item that stays on its region's axis until pulled 12px off |
