---
title: Virtualized list
description: A thin binding over gpui's uniform_list that pins the row height and hands the scrollbar a real handle.
prompt: Show the 50,000 search results in a virtualized list of 28px rows with a scrollbar.
---

```rust
use ui::{list, scroll};

div().relative().h(px(240.0))
    .child(list::virtual_list("rows", rows.len(), px(28.0), &self.rows_scroll, {
        let rows = rows.clone();
        move |range, _, _| range.map(|ix| row(&rows[ix])).collect()
    }))
    .child(scroll::scrollbar("rows-bar", list::scroll_handle(&self.rows_scroll), &self.rows_bar))
```

- The list fills its parent. A parent with no height of its own leaves the list empty, with no error.
- Every row is sized to `row_height`. For rows of varying height use `list::VariableList`.
- `list::scroll_handle` shares state with the list's handle; it is not a copy.
