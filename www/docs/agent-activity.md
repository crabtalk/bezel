---
title: Activity
description: A model working in public — the reasoning box pinned to its newest line, opening itself while the run streams and yielding the moment you press the header.
prompt: Show the model's reasoning in a box that stays scrolled to the newest line and opens while the run streams, until I click its header.
---

```rust
let open = self.thinking.get(self.running);   // widgets::Takeover: follows the run…
self.thinking.toggle(self.running);           // …on the header's click, until pressed

div().relative()
    .child(scroll::pane("reasoning", Axes::Vertical).track_scroll(&self.scroll).child(lines))
    .child(scroll::follow(&self.scroll, &self.follow))
```

- Pass the same `auto` value to `get` and `toggle`.
- `scroll::follow` goes over the same container as the pane it pins; scrolling up releases it, `FollowState::follow` re-pins.
- There is no activity component: the screen is [`scroll::follow`](/docs/follow), [`Takeover`](/docs/collapsible) and `div`s. Copy `apps/gallery/src/pages/agent/activity.rs`.
