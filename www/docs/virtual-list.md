---
title: Virtualized list
description: A thin binding over gpui's uniform_list that pins the row height and hands the scrollbar a real handle.
---

```rust
use ui::{list, scroll};

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

Thin on purpose — gpui already does the hard part.

## API

| | |
| --- | --- |
| `virtual_list(id, count, row_height, handle, render)` | `uniform_list` measures the *first* row and lays every other out at that height; hand it uneven rows and nothing errors, the content just overlaps. This applies the height to every row it returns. |
| `list::scroll_handle(handle)` | The reach to the real `ScrollHandle` inside a `UniformListScrollHandle`, named so a consumer does not have to find it in gpui's source. The clone shares state. |

The list fills its parent: one with no height of its own collapses to a single measured row and then nothing. Set your own size after the call — the later call wins.

gpui's other virtualizer, `list()`, takes rows of varying height and cannot carry a proportional scrollbar: a thumb's length is the visible share of a total height, and a variable-height list cannot know its total without measuring every row.
