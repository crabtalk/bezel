---
title: Pagination
description: For data that arrives in pages, not for lists that are long — one function turns (current, total) into the row you see.
---

A long list is answered by `scroll` and `list`, which will show ten thousand rows and build nine of them. A paginator earns its place only when the *data* is paged and the client cannot hold the whole set: an API that answers "page 4 of 87", a report with a fixed page size, a backend that will not stream. There the page number is not a scrolling affordance, it is the query.

The fiddly part is one function:

```rust
use bezel_ui::pagination::{self, Slot};

pagination::window(6, 20, 2)
// 1 … 4 5 [6] 7 8 … 20
```

```text
current = 6, total = 20   →   1 … 4 5 [6] 7 8 … 20
current = 2, total = 20   →   1 [2] 3 4 5 … 20
current = 3, total = 5    →   1 2 [3] 4 5
```

Pages are **1-based**, unlike the indices everywhere else in the library: a page number is a label a person reads, not an offset into a slice, and a paginator that can say "page 0" is a bug waiting to be filed. A `current` out of range is clamped rather than trusted — it arrives from the caller's state, and a paint is no place to panic.

Two rules earn their tests. A gap that hides exactly **one** page is worse than the page, so that page is shown instead — an ellipsis standing for a single number tells you less while taking the same room. And the window **slides** at the ends rather than shrinking, so walking to the last page never narrows the control under the pointer.

Which page you are on, how many there are and how to fetch one are all the caller's. Like the table's sort, this module reports and paints.
