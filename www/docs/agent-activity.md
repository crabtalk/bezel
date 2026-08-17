---
title: Activity
description: A model working in public — the reasoning box pinned to its newest line, opening itself while the run streams and yielding the moment you press the header.
---

The one screen an agent app cannot borrow from anything else, and the pattern that proved two pieces belonged in the library rather than in an agent crate.

`scroll::follow` pins the reasoning box to its newest line while the run writes into it. `widgets::Takeover` opens the section while that is happening and hands it over the moment you press the header:

```rust
let open = self.thinking.get(self.running);   // follows the run…
self.thinking.toggle(self.running);           // …until the header is pressed
```

Everything else on the page is a `div`. That is the finding: composing `LiveActivity` and `Thought` from the app this was extracted from produced exactly two library pieces, and both are general — a terminal wants follow-scroll, a build log wants a section that unfolds while it runs.

The source is at `apps/gallery/src/patterns/agent.rs`. Copy the file.

The answer zone is plain text on purpose. Streaming markdown is `bezel-markdown`'s job, and this page claims the *activity* zone works, not the answer zone — which is also why there is one exchange here rather than a transcript.
