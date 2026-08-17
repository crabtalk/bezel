---
title: Table
description: Columns declared once and handed to both the header and every row, so the two halves cannot drift apart.
---

Reach for a table when the third column of every row has to line up, because reading *down* it is the point. Most lists of things are records, and a record reads better as a `group_box` of `card_row`s.

```rust
use bezel_ui::table::{self, Align, Column, Width};

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

A header and a body that size their own cells drift apart the moment either changes, and nothing catches it — both halves look right on their own. So `row` zips its cells onto the same `Column` list the header used, and a cell is never sized where it is written. Cells shorter than columns is a bug in the caller: debug builds assert, release truncates rather than panicking at a user.

`Width` is `Fixed(px)` or `Flex(share)` — a share of what is left after the fixed columns have taken theirs. `Align` is `Start` or `End`; there is no `Center`, because in a column of data it is almost always wrong and offering it is how tables end up with one. `Column::align_end()` is what a number wants, so its digits line up by place value rather than by however wide the last one was.

Sorting is the caller's. `next_sort` says what a click means, the caller sorts its own rows, and the module paints the arrow:

```rust
let sort = table::next_sort(self.sort, column);
```

The sorted column reverses; any other column starts ascending. Inheriting the previous column's direction would mean clicking a fresh heading can sort it descending, which reads as the table ignoring the click.

Nothing here holds data, so nothing here can hold it out of date.
