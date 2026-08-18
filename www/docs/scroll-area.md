---
title: Scroll area
description: A styled bar over gpui's own scroll handle — the caller keeps the scrolling container, the bar just reports on it.
---

gpui scrolls a `div` perfectly well and draws nothing while it does. This is that bar, and only that bar:

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

The bar must span the container it reports on — its track *is* the viewport. A wrapper that swallowed the content would have to re-implement layout for it, which is why there is none.

`ScrollbarState` is a field on your view. It holds where in the thumb a drag was grabbed, shaped like gpui's `ScrollHandle` — an `Rc` cell that mutates through `&self` — so the bar carries its whole gesture without the view wiring a single listener. Without it the thumb would jump its middle to the pointer on every press.

The bar is an overlay, not a gutter, so one arriving or leaving never reflows the content beneath it. It shows nothing when the content fits, and it takes no `&Theme`: a scrollbar is a neutral overlay, so the thumb is `ink`, which follows the appearance on its own.

The geometry is two pure functions, and both of gpui's conventions are easy to get backwards: `max_offset` is the *overflow* — content minus viewport, not content — and `offset` is **negative** as you scroll down.

```rust
scroll::thumb(viewport, max_offset, offset, scroll::MIN_THUMB) // -> Option<Range<Pixels>>
scroll::offset_for_thumb(top, viewport, max_offset, size)      // the inverse
```

`thumb` answers `None` when there is nothing to scroll, when the viewport is zero — the frame before layout has run — and when the thumb would be shorter than `MIN_THUMB` could travel in.
