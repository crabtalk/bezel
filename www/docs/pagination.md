---
title: Pagination
description: For data that arrives in pages, not for lists that are long — one function turns (current, total) into the row you see.
prompt: Add page controls under the search results, which the API returns 50 at a time, showing "1 … 4 5 [6] 7 8 … 20".
---

```rust
use ui::{icons::glyph, pagination::{self, Slot}};

pagination::pagination()
    .child(div().id("prev").on_click(prev).child(pagination::step(&theme, glyph::ChevronLeft, page > 1)))
    .children(pagination::window(page, total, 2).into_iter().enumerate().map(|(slot, entry)| match entry {
        Slot::Gap => pagination::ellipsis(&theme).into_any_element(),
        Slot::Page(n) => pagination::page_button(&theme, n, n == page)
            .id(("page", slot))
            .on_click(cx.listener(move |view, _, _, cx| view.go_to_page(n, cx)))
            .into_any_element(),
    }))
    .child(div().id("next").on_click(next).child(pagination::step(&theme, glyph::ChevronRight, page < total)))
```

- Pages are 1-based. `window` clamps an out-of-range `current`.
- The caller owns the page number and every click, including ignoring a click on a disabled step.
- For a list that is only long, use [`scroll`](/docs/scroll-area) and [`list`](/docs/virtual-list) instead.
