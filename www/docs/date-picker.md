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

`today` comes from the app. bezel carries no clock, and the only thing that knows which day it is where you are is the app that has a time source.

`Date` is bezel's own, deliberately. chrono is already in the graph under gpui, so taking it would cost nothing to compile — and would make it a *public* dependency, so a consumer declaring its own chrono would end up with two incompatible ones. That is the split-graph failure `bezel::gpui` exists to prevent, and it buys nothing here: a picker needs no timezones, no parsing and no formatting. It needs the civil calendar, which is pure and testable without a window.

`Date::new(year, month, day)` is checked and answers `None` unless the day exists, so 29 February depends on the year — which is the whole point of asking. Fields are private and ordering is chronological, so nothing downstream ever has to ask whether a date is real.

A month is always drawn in six rows:

```rust
date::month_grid(month, date::Weekday::Monday) // [Date; 42]
```

Six even for a February that fits in four, so the card never changes height as you page — a popover that resizes under the pointer moves the day you were about to click. The leading and trailing cells are real dates from the neighbouring months rather than blanks, which makes `cell.month() != month.month()` the only test a cell needs and leaves clicking one meaningful.

Arrows walk days and weeks because the grid is two-dimensional, `pageup`/`pagedown` page months — the chords a browser's own date input uses — and the cursor is a single `Date`, so walking off the end of a month and paging to the next are the same operation and cannot disagree about where you are.

`CalendarEvent::Selected` fires on choosing a day, never on moving the cursor over one.
