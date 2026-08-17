---
title: Follow scroll
description: Stay pinned to the bottom while the reader leaves it there, and get out of the way the moment they scroll up.
---

Drop it in beside the scrollbar, over the same container:

```rust
use bezel_ui::scroll;

div().relative()
    .child(div().id("log").size_full().overflow_y_scroll().track_scroll(&self.scroll).child(rows))
    .child(scroll::follow(&self.scroll, &self.follow))
    .child(scroll::scrollbar("log-bar", &self.scroll, &self.bar))
```

Telling appended content from a user scroll is the whole problem, and neither is an event to subscribe to — both surface as the same handle reading differently than last frame. The overflow is what separates them: if it changed, the content grew, and the pin stays as the user last set it; if it did not, the offset moved because the *user* moved it, and being at the end is what re-pins. So scrolling up releases and scrolling back down re-attaches, with no gesture to hook.

`FollowState` starts pinned — a transcript or a log opens on its newest line — and `state.follow()` re-pins it from a "jump to bottom" button.

`scroll::at_bottom(max_offset, offset, slack)` is the predicate, exposed because a jump-to-bottom pill needs the same answer the follow element uses. Content that fits is always at the bottom: there is nowhere else to be, and answering `false` would unpin an empty log. The slack matters — a wheel lands on fractional offsets and a re-layout can move the end by a hair, and without it a view would unpin itself over a rounding error.

The correction lands a frame late, since the scrolling div was laid out with the old offset before this runs, which is why the element asks for that frame. At a streaming cadence it is invisible, and it converges: once pinned and at the end, nothing is requested.
