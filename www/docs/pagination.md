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

```rust
// ui::pagination

/// A place in the row: a page you can go to, or the mark for pages skipped.
pub enum Slot { Page(usize), Gap }

/// The pages to show, keeping `around` either side. `current` out of range is
/// clamped rather than trusted — a paint is no place to panic.
pub fn window(current: usize, total: usize, around: usize) -> Vec<Slot>;

pub fn pagination() -> gpui::Div;
pub fn page_button(theme: &Theme, page: usize, current: bool) -> gpui::Div;
pub fn ellipsis(theme: &Theme) -> gpui::Div;

/// Previous / next.
pub fn step(theme: &Theme, icon: impl Into<Icon>, enabled: bool) -> gpui::Div;

// ...
```
