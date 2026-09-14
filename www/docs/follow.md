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

| | |
| --- | --- |
| `follow(handle, state)` | Drop it in beside the scrollbar, over the same container. |
| `FollowState` | Starts pinned — a transcript opens on its newest line. `state.follow()` re-pins from a "jump to bottom" button. |
| `at_bottom(max_offset, offset, slack)` | The predicate, exposed because a jump-to-bottom pill needs the same answer. Content that fits is always at the bottom. |

The slack matters: a wheel lands on fractional offsets, and without it a view would unpin itself over a rounding error. The correction lands a frame late and converges — once pinned and at the end, nothing is requested.
