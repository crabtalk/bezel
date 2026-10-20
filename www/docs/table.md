---
title: Table
description: Columns declared once and handed to both the header and every row, so the two halves cannot drift apart.
prompt: Show the deployments as a table with Name, Region and a right-aligned Duration column, sortable by clicking a heading.
---

```rust
use ui::table::{self, Column, Width};

let columns = [Column::new("Name", Width::Flex(1.0)), Column::new("Duration", Width::Fixed(px(96.0))).align_end()];

table::table(&theme)
    .child(table::header(&theme).children(columns.iter().enumerate().map(|(index, column)| {
        table::header_cell(&theme, column, self.sort.filter(|s| s.column == index).map(|s| s.ascending))
            .id(("column", index))
            .on_click(cx.listener(move |view, _, _, cx| view.sort = Some(table::next_sort(view.sort, index))))
    })))
    .children(rows.iter().enumerate().map(|(index, item)| {
        table::row(&theme, &columns, index == 0, false, vec![item.name.clone().into_any_element(), item.duration.clone().into_any_element()])
    }))
```

- Pass the same `Column` list to the header and every `row`, with one cell per column.
- The table holds no data and does not sort: `next_sort` says what a click means, and the caller sorts its rows.
- For records rather than tuples, a `group_box` of `card_row`s reads better.
