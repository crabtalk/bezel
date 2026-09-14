---
title: Scroll area
description: A styled bar over gpui's own scroll handle — the caller keeps the scrolling container, the bar just reports on it.
---

```rust
use ui::scroll;

div().relative()                                // the bar is absolute in here
    .child(
        div()
            .id("pane")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)         // gpui's handle, the app's field
            .child(content),
    )
    .child(scroll::scrollbar("pane-bar", &self.scroll, &self.scroll_bar))
```

The bar must span the container it reports on — its track *is* the viewport.

## Geometry

```rust
scroll::thumb(viewport, max_offset, offset, scroll::MIN_THUMB) // -> Option<Range<Pixels>>
scroll::offset_for_thumb(top, viewport, max_offset, size)      // the inverse
```

Both of gpui's conventions are easy to get backwards: `max_offset` is the *overflow* — content minus viewport — and `offset` is **negative** as you scroll down.

## API

| | |
| --- | --- |
| `scrollbar(id, handle, state)` | An overlay, not a gutter, so one arriving never reflows the content. Shows nothing when the content fits. |
| `ScrollbarState` | A field on your view holding where in the thumb a drag was grabbed — without it the thumb jumps its middle to the pointer on every press. |
| `thumb(..)` | `None` when there is nothing to scroll, when the viewport is zero, or when the thumb would be shorter than `MIN_THUMB`. |

It takes no `&Theme`: a scrollbar is a neutral overlay, so the thumb is `ink` and follows the appearance on its own.
