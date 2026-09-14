---
title: Pagination
description: For data that arrives in pages, not for lists that are long — one function turns (current, total) into the row you see.
---

```rust
use ui::pagination::{self, Slot};

pagination::window(6, 20, 2)
// 1 … 4 5 [6] 7 8 … 20
```

Reach for it only when the *data* is paged — an API answering "page 4 of 87". A list that is merely long is `scroll` and `list`.

## Windowing

```text
current = 6, total = 20   →   1 … 4 5 [6] 7 8 … 20
current = 2, total = 20   →   1 [2] 3 4 5 … 20
current = 3, total = 5    →   1 2 [3] 4 5
```

Pages are 1-based. A gap that would hide exactly one page shows the page instead, and the window slides at the ends rather than shrinking.

## API

| | |
| --- | --- |
| `window(current, total, around)` | The `Slot`s to paint; `current` out of range is clamped, not trusted. |
| `Slot` | `Page(usize)` or `Gap`. |
| `pagination()` | The row container. |
| `page_button(theme, page, current)` | One page. |
| `ellipsis(theme)` | The gap mark. |
| `step(theme, icon, enabled)` | Previous / next. |
