---
title: Date picker
description: A select's closed face over an anchored month grid, on a civil calendar of bezel's own — sixty pure lines, no chrono in your graph.
prompt: Add a due-date field to the task form that opens a month calendar and saves the picked day.
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

- `today` comes from the app: bezel has no clock.
- `Date` is bezel's own, not chrono's. `Date::new` returns `None` for a day that does not exist.
- `date::month_grid` gives the 42 cells to draw a grid of your own.
