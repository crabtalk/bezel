---
title: Follow scroll
description: Stay pinned to the bottom while the reader leaves it there, and get out of the way the moment they scroll up.
prompt: Keep the build log pinned to its newest line as output streams in, unless I've scrolled up, and show a "jump to latest" button when I have.
---

```rust
use ui::scroll::{self, Axes};

div().relative()
    .child(scroll::pane("log", Axes::Vertical).size_full().track_scroll(&self.scroll).child(rows))
    .child(scroll::follow(&self.scroll, &self.follow))
    .child(scroll::scrollbar("log-bar", &self.scroll, &self.bar))

// jump to latest, shown while !self.follow.following()
self.follow.follow();
```

- `follow` goes beside the scrollbar, over the same container, and `FollowState` is a field on the view. It starts pinned.
- Scrolling up releases the pin and scrolling back to the end restores it; there is no gesture to wire.
- The correction lands one frame after the content grows.
