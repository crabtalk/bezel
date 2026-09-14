---
title: Date picker
description: A select's closed face over an anchored month grid, on a civil calendar of bezel's own — sixty pure lines, no chrono in your graph.
---

```rust
use ui::date::{self, Calendar, CalendarEvent, Date};

date::init(cx);   // once, at startup

let picker = cx.new(|cx| Calendar::new(today, cx));

cx.subscribe(&picker, |_, _, event, _| match event {
    CalendarEvent::Selected(date) => { /* the chosen day */ }
})
.detach();
```

`today` comes from the app: bezel carries no clock.

## The grid

```rust
date::month_grid(month, date::Weekday::Monday) // [Date; 42]
```

Always six rows, even for a February that fits in four — a popover that resizes under the pointer moves the day you were about to click.

## API

```rust
// ui::date

pub fn init(cx: &mut App);

impl Calendar {
    pub fn new(today: Date, cx: &mut Context<Self>) -> Self;

    // ...
}

/// Fires on choosing a day, never on moving the cursor over one.
pub enum CalendarEvent { Selected(Date) }

impl Date {
    /// Checked; `None` unless the day exists, so 29 February depends on the
    /// year. Fields are private and ordering is chronological.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self>;

    // ...
}

/// Always six rows. The leading and trailing cells are real dates from the
/// neighbouring months, so `cell.month() != month.month()` is the only test a
/// cell needs.
pub fn month_grid(month: Date, start: Weekday) -> [Date; 42];
```

Arrows walk days and weeks, `pageup`/`pagedown` page months. The cursor is a single `Date`, so walking off the end and paging are the same operation.

`Date` is bezel's own deliberately: chrono is already under gpui, but taking it would make it a *public* dependency and a consumer with its own would end up with two.
