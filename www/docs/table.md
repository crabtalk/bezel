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

```rust
// ui::table

pub fn table(theme: &Theme) -> gpui::Div;
pub fn header(theme: &Theme) -> gpui::Div;
pub fn header_cell(theme: &Theme, column: &Column, sorted: Option<bool>) -> gpui::Div;

/// Zips cells onto the same `Column` list the header used, so a cell is never
/// sized where it is written. Too few cells asserts in debug, truncates in
/// release rather than panicking at a user.
pub fn row(
    theme: &Theme,
    columns: &[Column],
    first: bool,
    selected: bool,
    cells: Vec<AnyElement>,
) -> gpui::Div;

pub enum Width {
    Fixed(f32),
    /// A share of what the fixed columns leave.
    Flex(f32),
}

/// No `Center`: in a column of data it is almost always wrong.
pub enum Align { Start, End }

impl Column {
    /// What a number wants, so digits line up by place value.
    pub fn align_end(self) -> Self;

    // ...
}

/// Says what a click means; the caller sorts its own rows and this paints the
/// arrow. The sorted column reverses, any other starts ascending.
pub fn next_sort(current: Option<Sort>, column: usize) -> Sort;
```

Nothing here holds data, so nothing here can hold it out of date.
