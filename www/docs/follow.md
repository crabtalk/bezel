---
title: Follow scroll
description: Stay pinned to the bottom while the reader leaves it there, and get out of the way the moment they scroll up.
---

```rust
use ui::scroll;

div().relative()
    .child(div().id("log").size_full().overflow_y_scroll().track_scroll(&self.scroll).child(rows))
    .child(scroll::follow(&self.scroll, &self.follow))
    .child(scroll::scrollbar("log-bar", &self.scroll, &self.bar))
```

The overflow is what tells appended content from a user scroll: if it changed, the content grew and the pin stays as the user set it; if it did not, the user moved it, and being at the end re-pins. Scrolling up releases and scrolling back down re-attaches, with no gesture to hook.

## API

```rust
// ui::scroll

/// Drop it in beside the scrollbar, over the same container.
pub fn follow(handle: &ScrollHandle, state: &FollowState) -> gpui::AnyElement;

/// Starts pinned — a transcript opens on its newest line.
pub struct FollowState(Rc<Cell<(bool, Pixels)>>);

impl FollowState {
    /// Re-pin, from a "jump to bottom" button.
    pub fn follow(&self);

    // ...
}

/// The predicate, exposed because a jump-to-bottom pill needs the same answer.
/// Content that fits is always at the bottom.
pub fn at_bottom(max_offset: Pixels, offset: Pixels, slack: Pixels) -> bool;
```

The slack matters: a wheel lands on fractional offsets, and without it a view would unpin itself over a rounding error. The correction lands a frame late and converges — once pinned and at the end, nothing is requested.
