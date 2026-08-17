---
title: Virtualized list
description: A thin binding over gpui's uniform_list that pins the row height and hands the scrollbar a real handle.
---

```rust
use bezel_ui::{list, scroll};

div().relative().h(px(240.0))
    .child(list::virtual_list("rows", rows.len(), px(28.0), &self.rows_scroll, {
        let rows = rows.clone();
        move |range, _, _| range.map(|ix| row(&rows[ix])).collect()
    }))
    .child(scroll::scrollbar(
        "rows-bar",
        &list::scroll_handle(&self.rows_scroll),
        &self.rows_bar,
    ))
```

Thin on purpose — gpui already does the hard part. The module exists for two things it can guarantee that a caller otherwise has to know.

**The row height.** `uniform_list` measures the *first* row it renders and lays every other one out at that height. Hand it rows of different heights and nothing errors: the content simply overlaps at a size nobody chose. `virtual_list` takes the height and applies it to every row it hands back.

**The scroll handle.** A `UniformListScrollHandle` wraps a real `ScrollHandle`, and the bar's geometry is all there — behind `handle.0.borrow().base_handle`, which is not something a consumer should have to find by reading gpui's source. `list::scroll_handle` is that reach, named. The clone shares state rather than copying it, so the bar reports on the list the list actually scrolls.

The list fills its parent. A virtualized list is bounded by definition, and one with no height of its own collapses — a collapsed list builds a single row to measure and then nothing, which looks like an empty box with no error and no clue. Set your own size after the call if you want otherwise; the later call wins.

gpui's other virtualizer, `list()`, handles rows of varying height and cannot carry a proportional scrollbar: `ListState` speaks in `ListOffset { item_ix, offset_in_item }` — logical position, not pixels — with no maximum offset and no viewport. A thumb's length is the visible share of a total height, and a variable-height list cannot know its total without measuring every row, which is the work virtualization exists to skip.
