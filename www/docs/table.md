---
title: Table
description: Columns declared once and handed to both the header and every row, so the two halves cannot drift apart.
---

```rust
use ui::table::{self, Align, Column, Width};

const COLUMNS: &[Column] = ..;   // one declaration

table::table(&theme)
    .child(table::header(&theme).children(COLUMNS.iter().enumerate().map(|(index, column)| {
        table::header_cell(&theme, column, sorted_direction(index))
            .id(("column", index))
            .on_click(cx.listener(move |view, _, _, cx| view.sort_by(index, cx)))
    })))
    .children(rows.iter().enumerate().map(|(index, item)| {
        table::row(&theme, COLUMNS, index == 0, false, vec![
            item.name.clone().into_any_element(),
            item.kind.clone().into_any_element(),
        ])
    }))
```

Reach for a table when the third column of every row has to line up. Most lists of things are records, and a record reads better as a `group_box` of `card_row`s.

## Sorting

```rust
let sort = table::next_sort(self.sort, column);
```

The sorted column reverses; any other starts ascending. Inheriting the previous direction would let a fresh heading sort descending, which reads as the table ignoring the click.

## API

| | |
| --- | --- |
| `row(theme, columns, first, selected, cells)` | Zips cells onto the same `Column` list the header used; a cell is never sized where it is written. Too few cells asserts in debug, truncates in release. |
| `Width` | `Fixed(px)` or `Flex(share)` — a share of what the fixed columns leave. |
| `Align` | `Start` or `End`. No `Center`: in a column of data it is almost always wrong. |
| `Column::align_end()` | What a number wants, so digits line up by place value. |
| `next_sort(sort, column)` | Says what a click means; the caller sorts its own rows and this paints the arrow. |

Nothing here holds data, so nothing here can hold it out of date.
