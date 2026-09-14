---
title: Activity
description: A model working in public — the reasoning box pinned to its newest line, opening itself while the run streams and yielding the moment you press the header.
---

```rust
let open = self.thinking.get(self.running);   // follows the run…
self.thinking.toggle(self.running);           // …until the header is pressed
```

Two library pieces carry the screen: [`scroll::follow`](/docs/follow) pins the reasoning box to its newest line while the run writes into it, and [`widgets::Takeover`](/docs/collapsible) opens the section while that is happening. Everything else is a `div`.

Both are general — a terminal wants follow-scroll, a build log wants a section that unfolds while it runs.

The source is at `apps/gallery/src/patterns/agent.rs`. Copy the file.
