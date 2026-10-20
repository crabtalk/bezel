---
title: Scroll area
description: A styled bar over gpui's own scroll handle — the caller keeps the scrolling container, the bar just reports on it.
prompt: Make the settings pane scroll vertically and show a scrollbar over it while it overflows.
---

```rust
use ui::scroll::{self, Axes, ScrollbarState};

// in the view's constructor: scroll_bar: ScrollbarState::new(Painter::of(cx))
div().relative()                                // the bar is absolute in here
    .child(
        scroll::pane("pane", Axes::Vertical)
            .size_full()
            .track_scroll(&self.scroll)         // a gpui ScrollHandle field
            .child(content),
    )
    .child(scroll::scrollbar("pane-bar", &self.scroll, &self.scroll_bar))
```

- The bar must span the container it reports on; its track is the viewport.
- Keep `ScrollHandle` and `ScrollbarState` as fields on the view, not built per render.
- `scroll::thumb` and `scroll::offset_for_thumb` take gpui's conventions: `max_offset` is the overflow (content minus viewport) and `offset` is negative as you scroll down.
